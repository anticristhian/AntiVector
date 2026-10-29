use av_core::{Control, CoreError, Params, RgbaImage, Stage};
use av_session::{JobEvent, Session};
use std::sync::{atomic::Ordering, mpsc, Arc};
use std::time::{Duration, Instant};

fn image() -> RgbaImage {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/images/logo_original_1.png");
    let image = image::open(root).unwrap().to_rgba8();
    RgbaImage::new(
        image.width(),
        image.height(),
        image.pixels().map(|p| p.0).collect(),
    )
    .unwrap()
}

#[test]
fn cache_dependencies_match_the_pure_pipeline_and_recover_from_cancellation() {
    let image = image();
    let session = Session::new(image.clone()).unwrap();
    let mut p = Params {
        min_region_area: 4,
        ..Params::default()
    };
    let first = session.vectorize(&p, &Control::default()).unwrap();
    assert_eq!(
        *first,
        av_core::vectorize(&image, &p, &Control::default()).unwrap()
    );
    let before = session.counters().unwrap().executions;
    assert!(Arc::ptr_eq(
        &first,
        &session.vectorize(&p, &Control::default()).unwrap()
    ));
    assert_eq!(session.counters().unwrap().executions, before);
    p.smoothness = 0.7;
    p.fit_tolerance_px = 1.14;
    session.vectorize(&p, &Control::default()).unwrap();
    let after = session.counters().unwrap().executions;
    assert_eq!(&before[..7], &after[..7]);
    assert_eq!(after[7], before[7] + 1);
    assert_eq!(after[8], before[8] + 1);
    p.colors = av_core::ColorCount::Fixed(4);
    let ctl = Control::default();
    let flag = ctl.cancel.clone();
    let ctl = Control {
        progress: Arc::new(move |stage, _| {
            if stage == Stage::ResolveAa {
                flag.store(true, Ordering::Relaxed);
            }
        }),
        ..ctl
    };
    assert_eq!(session.vectorize(&p, &ctl), Err(CoreError::Cancelled));
    let recovered = session.vectorize(&p, &Control::default()).unwrap();
    assert_eq!(
        *recovered,
        av_core::vectorize(&image, &p, &Control::default()).unwrap()
    );
    let before = session.counters().unwrap().executions;
    p.detail = 0.8;
    p.min_region_area = 2;
    let updated = session.vectorize(&p, &Control::default()).unwrap();
    let after = session.counters().unwrap().executions;
    assert_eq!(&before[..5], &after[..5]);
    assert!(after[5] > before[5]);
    assert_eq!(
        *updated,
        av_core::vectorize(&image, &p, &Control::default()).unwrap()
    );
}

#[test]
fn worker_cancels_under_200ms_and_preserves_the_completed_document() {
    let source = RgbaImage::new(
        512,
        512,
        (0..512 * 512)
            .map(|i| {
                [
                    (i % 256) as u8,
                    ((i / 512) % 256) as u8,
                    (i % 137) as u8,
                    255,
                ]
            })
            .collect(),
    )
    .unwrap();
    let session = Arc::new(Session::new(source).unwrap());
    let mut initial = Params {
        min_region_area: 4,
        colors: av_core::ColorCount::Fixed(2),
        ..Params::default()
    };
    initial.experimental.quality_v5 = false;
    initial.experimental.curve_fit = false;
    let completed = session.vectorize(&initial, &Control::default()).unwrap();
    let mut p = Params {
        min_region_area: 4,
        ..Params::default()
    };
    p.experimental.quality_v5 = false;
    let (tx, rx) = mpsc::channel();
    let sink = Arc::new(move |event| {
        tx.send(event).unwrap();
    });
    session.start(p, sink).unwrap();
    loop {
        if matches!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            JobEvent::Progress {
                stage: Stage::Quantize,
                ..
            }
        ) {
            break;
        }
    }
    let start = Instant::now();
    session.cancel().unwrap();
    loop {
        if let JobEvent::Error { error, .. } = rx.recv_timeout(Duration::from_secs(10)).unwrap() {
            assert_eq!(error, CoreError::Cancelled);
            break;
        }
    }
    let elapsed = start.elapsed();
    println!("worker cancellation: {elapsed:?}");
    assert!(elapsed < Duration::from_millis(200));
    assert!(Arc::ptr_eq(&completed, &session.latest().unwrap().unwrap()));
}

#[test]
fn a_new_job_supersedes_old_results() {
    let session = Arc::new(Session::new(image()).unwrap());
    let (tx, rx) = mpsc::channel();
    let gate = Arc::new(std::sync::Barrier::new(2));
    let worker_gate = gate.clone();
    let sink = Arc::new(move |event| {
        let pause = matches!(
            event,
            JobEvent::Progress {
                job_id: 1,
                stage: Stage::Preprocess,
                fraction: 0.0
            }
        );
        tx.send(event).unwrap();
        if pause {
            worker_gate.wait();
        }
    });
    let a = Params {
        min_region_area: 4,
        ..Params::default()
    };
    let mut b = a.clone();
    b.smoothness = 0.8;
    let old = session.start(a, sink.clone()).unwrap();
    loop {
        if matches!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            JobEvent::Progress {
                job_id: 1,
                stage: Stage::Preprocess,
                fraction: 0.0
            }
        ) {
            break;
        }
    }
    let new = session.start(b.clone(), sink).unwrap();
    gate.wait();
    assert!(new > old);
    loop {
        if let JobEvent::Done { job_id, doc } = rx.recv_timeout(Duration::from_secs(20)).unwrap() {
            assert_eq!(job_id, new);
            assert_eq!(
                *doc,
                av_core::vectorize(&session.source(), &b, &Control::default()).unwrap()
            );
            break;
        }
    }
}

#[test]
fn preview_is_bounded_even_for_long_thin_images() {
    let image = RgbaImage::new(100_000, 1, vec![[10, 20, 30, 128]; 100_000]).unwrap();
    let preview = av_session::jobs::preview_raster(&image, 100, &Control::default()).unwrap();
    assert_eq!((preview.w, preview.h), (100, 1));
    assert!(preview.px.iter().all(|p| *p == [10, 20, 30, 128]));
    assert!(av_session::jobs::preview_raster(&image, 0, &Control::default()).is_err());
}
