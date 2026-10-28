//! Pure, reusable stage products and shared-geometry document assembly.

use crate::{
    assemble::{assemble, polygon_edges, VectorDoc},
    boundary::graph::BoundaryGraph,
    color::{LabelMap, Palette},
    segment::RegionMap,
    stages, Control, CoreError, Params, RgbaImage,
};
use std::{borrow::Cow, sync::Arc};

/// Intermediate F2 stage products, exposed for diagnostics and exact-coverage tests.
pub struct PolygonStages<'a> {
    pub preprocessed: Cow<'a, RgbaImage>,
    pub scale: u32,
    pub params: Params,
    pub detection: crate::detect::Detection,
    pub quant_palette: Palette,
    pub resolved_palette: Palette,
    pub palette: Palette,
    pub quantized: Arc<LabelMap>,
    pub resolved: Arc<LabelMap>,
    pub labeled: RegionMap,
    pub merged: RegionMap,
    pub graph: BoundaryGraph,
}

/// Run through exact topology extraction, retaining products for --dump-stages.
pub fn polygon_stages<'a>(
    image: &'a RgbaImage,
    params: &Params,
    ctl: &Control,
) -> Result<PolygonStages<'a>, CoreError> {
    ctl.check()?;
    image.validate(crate::raster::DEFAULT_PIXEL_LIMIT)?;
    params.validate()?;
    ctl.report(crate::Stage::Detect, 0.0)?;
    let mut detection = crate::detect(image);
    detection.is_pixel_art |= params.experimental.force_pixel_art;
    ctl.report(crate::Stage::Detect, 1.0)?;
    let (preprocessed, scale) = if params.experimental.quality_v5 {
        let prepared = stages::preprocess(image, params, &detection, ctl)?;
        (Cow::Owned(prepared.image), prepared.scale)
    } else {
        (Cow::Borrowed(image), 1)
    };
    let params = scaled_params(params, scale);
    let (quant_palette, quantized) = stages::quantize(&preprocessed, &params, ctl)?;
    let quantized = Arc::new(quantized);
    let (palette, resolved, mixture) = if params.experimental.quality_v5 {
        let aa = stages::resolve_aa(&preprocessed, &quant_palette, &quantized, &params, ctl)?;
        (aa.palette, Arc::new(aa.labels), Some(aa.mixture_pixels))
    } else {
        (quant_palette.clone(), quantized.clone(), None)
    };
    let labeled = stages::label(&resolved, Some(&preprocessed), ctl)?;
    let mut merged = if let Some(mask) = mixture {
        stages::merge_small_with_aa(&labeled, &palette, &params, Some(&preprocessed), &mask, ctl)?
    } else {
        stages::merge_small(&labeled, &palette, &params, Some(&preprocessed), ctl)?
    };
    let resolved_palette = palette;
    let palette = if params.experimental.quality_v5 {
        crate::color::compact_regions(&resolved_palette, &mut merged)?
    } else {
        resolved_palette.clone()
    };
    let graph = stages::extract_boundaries(&merged, ctl)?;
    Ok(PolygonStages {
        preprocessed,
        scale,
        params,
        detection,
        quant_palette,
        resolved_palette,
        resolved,
        palette,
        quantized,
        labeled,
        merged,
        graph,
    })
}

/// Convert user pixel-unit parameters into prepared-raster units without exceeding
/// validated fitting bounds; these are upper error limits, so clamping is stricter.
pub fn scaled_params(params: &Params, scale: u32) -> Params {
    let mut result = params.clone();
    result.min_region_area = result
        .min_region_area
        .saturating_mul(scale.saturating_mul(scale));
    result.fit_tolerance_px = (result.fit_tolerance_px * scale as f32).min(3.0);
    result.experimental.corner_recovery_radius =
        (result.experimental.corner_recovery_radius * f64::from(scale)).min(2.0);
    result
}

/// Restore original image coordinates once, on the canonical shared edges.
pub fn restore_scale(mut doc: VectorDoc, scale: u32) -> VectorDoc {
    let scale = f64::from(scale);
    doc.width /= scale;
    doc.height /= scale;
    let point = |p: &mut [f64; 2]| {
        p[0] /= scale;
        p[1] /= scale;
    };
    for edge in &mut doc.edges {
        point(&mut edge.start);
        for segment in &mut edge.segs {
            match segment {
                crate::assemble::Seg::Line(p) => point(p),
                crate::assemble::Seg::Cubic(a, b, p) => {
                    point(a);
                    point(b);
                    point(p);
                }
            }
        }
    }
    doc
}

/// Finish reusable segmentation products with fitting and assembly.
pub fn finish(result: &PolygonStages<'_>, ctl: &Control) -> Result<VectorDoc, CoreError> {
    let params = &result.params;
    let pixel_art = result.detection.is_pixel_art || params.experimental.force_pixel_art;
    let edges = if params.experimental.curve_fit {
        crate::fit::fit_edges_for_regions_at_scale(
            &result.graph,
            &result.merged,
            &result.palette,
            params,
            pixel_art,
            result.scale,
            ctl,
        )?
    } else {
        polygon_edges(&result.graph)
    };
    Ok(restore_scale(
        assemble(&result.merged, &result.graph, &result.palette, edges, ctl)?,
        result.scale,
    ))
}

/// Convert RGBA8 into shared vector geometry with stage-boundary cancellation.
pub fn vectorize(
    image: &RgbaImage,
    params: &Params,
    ctl: &Control,
) -> Result<VectorDoc, CoreError> {
    let result = polygon_stages(image, params, ctl)?;
    finish(&result, ctl)
}
