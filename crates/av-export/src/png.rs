//! Lossless RGBA PNG rasterized with resvg at the document's source resolution.
use crate::svg::SvgOptions;
use av_core::{CoreError, VectorDoc};

/// Rasterize exported paint paths on a transparent canvas. No external resources.
pub fn to_bytes(doc: &VectorDoc, options: &SvgOptions) -> Result<Vec<u8>, CoreError> {
    let svg = crate::svg::to_string_with_options(doc, options)?;
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
        .map_err(|e| CoreError::Internal(format!("SVG rasterization: {e}")))?;
    let size = tree.size().to_int_size();
    av_core::raster::validate_dimensions(
        size.width(),
        size.height(),
        av_core::raster::DEFAULT_PIXEL_LIMIT,
    )?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| CoreError::InvalidParams("PNG allocation".into()))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| CoreError::Internal(format!("PNG encoding: {e}")))
}
