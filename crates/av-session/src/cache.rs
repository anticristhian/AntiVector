//! Last-value stage cache with explicit parameter dependencies, no hash collisions.
use av_core::{
    assemble::FittedEdge,
    boundary::graph::BoundaryGraph,
    color::{LabelMap, Palette},
    detect::Detection,
    segment::RegionMap,
    stages, Control, CoreError, Params, RgbaImage, VectorDoc,
};
use std::sync::Arc;

#[derive(Clone)]
struct Resolved {
    palette: Arc<Palette>,
    labels: Arc<LabelMap>,
    mixture: Option<Arc<Vec<bool>>>,
}

/// Actual executions at each Stage discriminant; cache hits never increment it.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct CacheCounters {
    pub executions: [u64; 9],
}

/// Immutable source and stage products, reused only while their dependencies match.
pub struct StageCache {
    source: Arc<RgbaImage>,
    detection: Detection,
    key: Option<Params>,
    prepared: Option<(Arc<RgbaImage>, u32)>,
    quant: Option<(Arc<Palette>, Arc<LabelMap>)>,
    resolved: Option<Resolved>,
    labeled: Option<Arc<RegionMap>>,
    merged: Option<(Arc<RegionMap>, Arc<Palette>)>,
    graph: Option<Arc<BoundaryGraph>>,
    edges: Option<Arc<Vec<FittedEdge>>>,
    doc: Option<Arc<VectorDoc>>,
    counts: CacheCounters,
}

/// One shared segmentation checkpoint; history never duplicates its pixel buffers.
#[derive(Clone)]
pub struct SegmentationSnapshot {
    pub image: Arc<RgbaImage>,
    pub map: Arc<RegionMap>,
    pub palette: Arc<Palette>,
    pub scale: u32,
    pub pixel_art: bool,
}

impl SegmentationSnapshot {
    /// Rebuild all affected boundaries once, preserving shared geometry after edits.
    pub fn assemble(
        &self,
        map: &RegionMap,
        palette: &Palette,
        params: &Params,
        ctl: &Control,
    ) -> Result<VectorDoc, CoreError> {
        let graph = stages::extract_boundaries(map, ctl)?;
        let params = av_core::pipeline::scaled_params(params, self.scale);
        let edges = if params.experimental.curve_fit {
            av_core::fit::fit_edges_for_regions_at_scale(
                &graph,
                map,
                palette,
                &params,
                self.pixel_art,
                self.scale,
                ctl,
            )?
        } else {
            av_core::assemble::polygon_edges(&graph)
        };
        Ok(av_core::pipeline::restore_scale(
            av_core::assemble::assemble(map, &graph, palette, edges, ctl)?,
            self.scale,
        ))
    }
}

fn changed(a: &Params, b: &Params) -> usize {
    let x = &a.experimental;
    let y = &b.experimental;
    if a.denoise != b.denoise
        || a.transparency_alpha_cut != b.transparency_alpha_cut
        || x.quality_v5 != y.quality_v5
        || x.denoise_kind != y.denoise_kind
        || x.upscale_small != y.upscale_small
        || x.force_pixel_art != y.force_pixel_art
    {
        1
    } else if a.mode != b.mode
        || a.colors != b.colors
        || a.palette_lock != b.palette_lock
        || a.seed != b.seed
        || x.quantizer != y.quantizer
    {
        2
    } else if a.antialias_resolve != b.antialias_resolve {
        3
    } else if a.min_region_area != b.min_region_area || a.detail != b.detail {
        5
    } else if a.smoothness != b.smoothness
        || a.corner_threshold_deg != b.corner_threshold_deg
        || a.fit_tolerance_px != b.fit_tolerance_px
        || x.corner_recovery_radius != y.corner_recovery_radius
        || x.curve_fit != y.curve_fit
        || x.pixel_art_strategy != y.pixel_art_strategy
        || x.circular_loops != y.circular_loops
    {
        7
    } else {
        9
    }
}

impl StageCache {
    /// Bind a validated source for the lifetime of this cache.
    pub fn new(source: Arc<RgbaImage>) -> Result<Self, CoreError> {
        source.validate(av_core::raster::DEFAULT_PIXEL_LIMIT)?;
        let detection = av_core::detect(&source);
        let mut counts = CacheCounters::default();
        counts.executions[0] = 1;
        Ok(Self {
            source,
            detection,
            key: None,
            prepared: None,
            quant: None,
            resolved: None,
            labeled: None,
            merged: None,
            graph: None,
            edges: None,
            doc: None,
            counts,
        })
    }
    /// Counts only completed executions, useful for cache correctness and profiling.
    pub fn counters(&self) -> CacheCounters {
        self.counts.clone()
    }
    /// Snapshot after a successful run; its source and labels are immutable Arc data.
    pub fn segmentation(&self) -> Option<SegmentationSnapshot> {
        let (image, scale) = self.prepared.as_ref()?;
        let (map, palette) = self.merged.as_ref()?;
        Some(SegmentationSnapshot {
            image: image.clone(),
            map: map.clone(),
            palette: palette.clone(),
            scale: *scale,
            pixel_art: self.detection.is_pixel_art
                || self.key.as_ref()?.experimental.force_pixel_art,
        })
    }
    fn invalidate(&mut self, from: usize) {
        if from <= 1 {
            self.prepared = None;
        }
        if from <= 2 {
            self.quant = None;
        }
        if from <= 3 {
            self.resolved = None;
        }
        if from <= 4 {
            self.labeled = None;
        }
        if from <= 5 {
            self.merged = None;
        }
        if from <= 6 {
            self.graph = None;
        }
        if from <= 7 {
            self.edges = None;
        }
        if from <= 8 {
            self.doc = None;
        }
    }
    /// Run missing stages. A cancellation retains only completed, correctly keyed products.
    pub fn run(&mut self, p: &Params, ctl: &Control) -> Result<Arc<VectorDoc>, CoreError> {
        p.validate()?;
        ctl.check()?;
        let from = self.key.as_ref().map_or(1, |previous| changed(previous, p));
        self.invalidate(from);
        self.key = Some(p.clone());
        if self.prepared.is_none() {
            let value = if p.experimental.quality_v5 {
                let prepared = stages::preprocess(&self.source, p, &self.detection, ctl)?;
                (Arc::new(prepared.image), prepared.scale)
            } else {
                (self.source.clone(), 1)
            };
            self.prepared = Some(value);
            self.counts.executions[1] += 1;
        }
        let (image, scale) = self.prepared.as_ref().unwrap();
        let scale = *scale;
        let p = av_core::pipeline::scaled_params(p, scale);
        if self.quant.is_none() {
            let (palette, labels) = stages::quantize(image, &p, ctl)?;
            self.quant = Some((Arc::new(palette), Arc::new(labels)));
            self.counts.executions[2] += 1;
        }
        if self.resolved.is_none() {
            let (palette, labels) = self.quant.as_ref().unwrap();
            let resolved = if p.experimental.quality_v5 {
                let aa = stages::resolve_aa(image, palette, labels, &p, ctl)?;
                Resolved {
                    palette: Arc::new(aa.palette),
                    labels: Arc::new(aa.labels),
                    mixture: Some(Arc::new(aa.mixture_pixels)),
                }
            } else {
                Resolved {
                    palette: palette.clone(),
                    labels: labels.clone(),
                    mixture: None,
                }
            };
            self.resolved = Some(resolved);
            self.counts.executions[3] += 1;
        }
        let resolved = self.resolved.as_ref().unwrap();
        if self.labeled.is_none() {
            self.labeled = Some(Arc::new(stages::label(&resolved.labels, Some(image), ctl)?));
            self.counts.executions[4] += 1;
        }
        if self.merged.is_none() {
            let map = self.labeled.as_ref().unwrap();
            let mut merged = if let Some(mask) = &resolved.mixture {
                stages::merge_small_with_aa(map, &resolved.palette, &p, Some(image), mask, ctl)?
            } else {
                stages::merge_small(map, &resolved.palette, &p, Some(image), ctl)?
            };
            let palette = if p.experimental.quality_v5 {
                Arc::new(av_core::color::compact_regions(
                    &resolved.palette,
                    &mut merged,
                )?)
            } else {
                resolved.palette.clone()
            };
            self.merged = Some((Arc::new(merged), palette));
            self.counts.executions[5] += 1;
        }
        let (map, palette) = self.merged.as_ref().unwrap();
        if self.graph.is_none() {
            self.graph = Some(Arc::new(stages::extract_boundaries(map, ctl)?));
            self.counts.executions[6] += 1;
        }
        let graph = self.graph.as_ref().unwrap();
        if self.edges.is_none() {
            let edges = if p.experimental.curve_fit {
                av_core::fit::fit_edges_for_regions_at_scale(
                    graph,
                    map,
                    palette,
                    &p,
                    self.detection.is_pixel_art || p.experimental.force_pixel_art,
                    scale,
                    ctl,
                )?
            } else {
                av_core::assemble::polygon_edges(graph)
            };
            self.edges = Some(Arc::new(edges));
            self.counts.executions[7] += 1;
        }
        if self.doc.is_none() {
            let edges = self.edges.as_ref().unwrap().as_ref().clone();
            let doc = av_core::pipeline::restore_scale(
                av_core::assemble::assemble(map, graph, palette, edges, ctl)?,
                scale,
            );
            self.doc = Some(Arc::new(doc));
            self.counts.executions[8] += 1;
        }
        ctl.check()?;
        Ok(self.doc.as_ref().unwrap().clone())
    }
}
