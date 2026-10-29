//! Cooperative jobs: cancellation never waits for the worker's stage-cache lock.
use crate::cache::{CacheCounters, StageCache};
use av_core::{Control, CoreError, Params, RgbaImage, Stage, VectorDoc};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

/// Monotonic within a session; frontends must also check their session ID.
pub type JobId = u64;
/// Worker notifications. A preview cannot be exported as the final document.
pub enum JobEvent {
    Progress {
        job_id: JobId,
        stage: Stage,
        fraction: f32,
    },
    Preview {
        job_id: JobId,
        doc: Arc<VectorDoc>,
    },
    Done {
        job_id: JobId,
        doc: Arc<VectorDoc>,
    },
    Error {
        job_id: JobId,
        error: CoreError,
    },
}

/// A source, completed document and one current job with reusable stage products.
pub struct Session {
    source: Arc<RgbaImage>,
    cache: Mutex<StageCache>,
    latest: Mutex<Option<Arc<VectorDoc>>>,
    active: Mutex<Option<(JobId, Control)>>,
    next: AtomicU64,
}

fn poisoned<T>(_: std::sync::PoisonError<T>) -> CoreError {
    CoreError::Internal("session lock poisoned".into())
}

impl Session {
    /// Bind a decoded image. IO and thumbnail encoding belong to the caller.
    pub fn new(image: RgbaImage) -> Result<Self, CoreError> {
        let source = Arc::new(image);
        let cache = StageCache::new(source.clone())?;
        Ok(Self {
            source,
            cache: Mutex::new(cache),
            latest: Mutex::new(None),
            active: Mutex::new(None),
            next: AtomicU64::new(1),
        })
    }
    /// Original immutable raster for comparison/export/project storage.
    pub fn source(&self) -> Arc<RgbaImage> {
        self.source.clone()
    }
    /// Snapshot of completed-stage counters. Does not count cache hits.
    pub fn counters(&self) -> Result<CacheCounters, CoreError> {
        Ok(self.cache.lock().map_err(poisoned)?.counters())
    }
    /// Last completed full-resolution document; never replaced by preview/cancel/error.
    pub fn latest(&self) -> Result<Option<Arc<VectorDoc>>, CoreError> {
        Ok(self.latest.lock().map_err(poisoned)?.clone())
    }
    fn compute(&self, p: &Params, ctl: &Control) -> Result<Arc<VectorDoc>, CoreError> {
        ctl.check()?;
        self.cache.lock().map_err(poisoned)?.run(p, ctl)
    }
    /// Synchronous worker API, also useful to headless consumers and cache tests.
    pub fn vectorize(&self, p: &Params, ctl: &Control) -> Result<Arc<VectorDoc>, CoreError> {
        let doc = self.compute(p, ctl)?;
        *self.latest.lock().map_err(poisoned)? = Some(doc.clone());
        Ok(doc)
    }
    /// Signal cancellation immediately, independently of stage-cache ownership.
    pub fn cancel(&self) -> Result<(), CoreError> {
        if let Some((_, ctl)) = &*self.active.lock().map_err(poisoned)? {
            ctl.cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
    fn current(&self, id: JobId) -> bool {
        self.active
            .lock()
            .is_ok_and(|a| a.as_ref().is_some_and(|(current, _)| *current == id))
    }
    fn publish(&self, id: JobId, ctl: &Control, doc: &Arc<VectorDoc>) -> Result<bool, CoreError> {
        let active = self.active.lock().map_err(poisoned)?;
        if !active.as_ref().is_some_and(|(current, _)| *current == id)
            || ctl.cancel.load(Ordering::Relaxed)
        {
            return Ok(false);
        }
        *self.latest.lock().map_err(poisoned)? = Some(doc.clone());
        Ok(true)
    }
    /// Cancel the previous job and run on a worker. Late events carry their old ID.
    pub fn start(
        self: &Arc<Self>,
        p: Params,
        notify: Arc<dyn Fn(JobEvent) + Send + Sync>,
    ) -> Result<JobId, CoreError> {
        self.start_with_preview_budget(p, 250_000, notify)
    }
    /// Harness-adjustable preview budget; full-resolution results remain identical.
    pub fn start_with_preview_budget(
        self: &Arc<Self>,
        p: Params,
        preview_pixels: usize,
        notify: Arc<dyn Fn(JobEvent) + Send + Sync>,
    ) -> Result<JobId, CoreError> {
        if preview_pixels == 0 {
            return Err(CoreError::InvalidParams("zero preview budget".into()));
        }
        p.validate()?;
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let sink = notify.clone();
        let ctl = Control {
            progress: Arc::new(move |stage, fraction| {
                sink(JobEvent::Progress {
                    job_id: id,
                    stage,
                    fraction,
                })
            }),
            ..Control::default()
        };
        {
            let mut active = self.active.lock().map_err(poisoned)?;
            if let Some((_, previous)) = active.as_ref() {
                previous.cancel.store(true, Ordering::Relaxed);
            }
            *active = Some((id, ctl.clone()));
        }
        let session = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                // A cooperative box preview is independent of the full-resolution cache.
                if session.source.px.len() > 1_000_000 {
                    let preview = preview_raster(&session.source, preview_pixels, &ctl)?;
                    let mut params = p.clone();
                    params.min_region_area = av_core::params::default_min_region_area(
                        params.detail,
                        preview.w,
                        preview.h,
                    );
                    let doc = av_core::vectorize(&preview, &params, &ctl)?;
                    if session.current(id) {
                        notify(JobEvent::Preview {
                            job_id: id,
                            doc: Arc::new(doc),
                        });
                    }
                }
                let doc = session.compute(&p, &ctl)?;
                ctl.check()?;
                if session.publish(id, &ctl, &doc)? {
                    notify(JobEvent::Done { job_id: id, doc });
                }
                Ok::<_, CoreError>(())
            })();
            if let Err(error) = result {
                if session.current(id) {
                    notify(JobEvent::Error { job_id: id, error });
                }
            }
            if let Ok(mut active) = session.active.lock() {
                if active.as_ref().is_some_and(|(current, _)| *current == id) {
                    *active = None;
                }
            }
        });
        Ok(id)
    }
}

/// Area averaging with bounded cancellation checks; no image-decoding dependencies.
pub fn preview_raster(
    image: &RgbaImage,
    limit: usize,
    ctl: &Control,
) -> Result<RgbaImage, CoreError> {
    if limit == 0 {
        return Err(CoreError::InvalidParams("zero preview budget".into()));
    }
    image.validate(av_core::raster::DEFAULT_PIXEL_LIMIT)?;
    let factor = ((image.px.len() as f64 / limit as f64).sqrt()).max(1.0);
    let mut w = (f64::from(image.w) / factor).floor().max(1.0) as u32;
    let mut h = (f64::from(image.h) / factor).floor().max(1.0) as u32;
    if u64::from(w) * u64::from(h) > limit as u64 {
        if w >= h {
            w = (limit as u64 / u64::from(h)).max(1) as u32;
        } else {
            h = (limit as u64 / u64::from(w)).max(1) as u32;
        }
    }
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            if pixels.len() % 1024 == 0 {
                ctl.check()?;
            }
            let x0 = (u64::from(x) * u64::from(image.w) / u64::from(w)) as u32;
            let x1 = ((u64::from(x + 1) * u64::from(image.w) / u64::from(w)) as u32).max(x0 + 1);
            let y0 = (u64::from(y) * u64::from(image.h) / u64::from(h)) as u32;
            let y1 = ((u64::from(y + 1) * u64::from(image.h) / u64::from(h)) as u32).max(y0 + 1);
            let mut sum = [0_u64; 4];
            let mut count = 0_u64;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    if count.is_multiple_of(4096) {
                        ctl.check()?;
                    }
                    let p = image.px[(sy * image.w + sx) as usize];
                    for ch in 0..3 {
                        sum[ch] += u64::from(p[ch]) * u64::from(p[3]);
                    }
                    sum[3] += u64::from(p[3]);
                    count += 1;
                }
            }
            pixels.push([
                sum[0].checked_div(sum[3]).unwrap_or(0) as u8,
                sum[1].checked_div(sum[3]).unwrap_or(0) as u8,
                sum[2].checked_div(sum[3]).unwrap_or(0) as u8,
                (sum[3] / count) as u8,
            ]);
        }
    }
    RgbaImage::new(w, h, pixels)
}
