//! Pure engine orchestration; F2 output is intentionally polygonal until F4.

use crate::{
    assemble::{assemble, polygon_edges, VectorDoc},
    boundary::graph::BoundaryGraph,
    color::{LabelMap, Palette},
    segment::RegionMap,
    stages, Control, CoreError, Params, RgbaImage,
};

/// Intermediate F2 stage products, exposed for diagnostics and exact-coverage tests.
pub struct PolygonStages {
    pub palette: Palette,
    pub quantized: LabelMap,
    pub labeled: RegionMap,
    pub merged: RegionMap,
    pub graph: BoundaryGraph,
}

/// Run through exact topology extraction, retaining products for --dump-stages.
pub fn polygon_stages(
    image: &RgbaImage,
    params: &Params,
    ctl: &Control,
) -> Result<PolygonStages, CoreError> {
    ctl.check()?;
    let (palette, quantized) = stages::quantize(image, params, ctl)?;
    let labeled = stages::label(&quantized, Some(image), ctl)?;
    let merged = stages::merge_small(&labeled, &palette, params, Some(image), ctl)?;
    let graph = stages::extract_boundaries(&merged, ctl)?;
    Ok(PolygonStages {
        palette,
        quantized,
        labeled,
        merged,
        graph,
    })
}

/// Convert RGBA8 to a vector document. F2 retains pixel-grid polygon geometry.
/// AA resolution, preprocessing and curve fitting are added at their planned phases.
pub fn vectorize(
    image: &RgbaImage,
    params: &Params,
    ctl: &Control,
) -> Result<VectorDoc, CoreError> {
    let result = polygon_stages(image, params, ctl)?;
    let edges = if params.experimental.curve_fit {
        crate::stages::fit_edges_for_regions(
            &result.graph,
            &result.merged,
            &result.palette,
            params,
            crate::detect(image).is_pixel_art,
            ctl,
        )?
    } else {
        polygon_edges(&result.graph)
    };
    assemble(&result.merged, &result.graph, &result.palette, edges, ctl)
}
