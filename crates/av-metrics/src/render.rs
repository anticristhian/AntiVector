//! Reproducible AA fidelity and non-AA coverage renders using resvg.

use av_core::{CoreError, RgbaImage};

/// Rasterize at original SVG dimensions. CrispEdges disables path anti-aliasing.
pub fn svg(svg: &str, antialias: bool) -> Result<RgbaImage, CoreError> {
    let options = resvg::usvg::Options {
        shape_rendering: if antialias {
            resvg::usvg::ShapeRendering::GeometricPrecision
        } else {
            resvg::usvg::ShapeRendering::CrispEdges
        },
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(svg, &options)
        .map_err(|e| CoreError::InvalidParams(e.to_string()))?;
    let size = tree.size().to_int_size();
    av_core::raster::validate_dimensions(
        size.width(),
        size.height(),
        av_core::raster::DEFAULT_PIXEL_LIMIT,
    )?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| CoreError::Internal("render allocation".into()))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let px = pixmap
        .pixels()
        .iter()
        .map(|p| {
            let p = p.demultiply();
            [p.red(), p.green(), p.blue(), p.alpha()]
        })
        .collect();
    RgbaImage::new(size.width(), size.height(), px)
}
