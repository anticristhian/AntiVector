//! Vector document writers; exporters must preserve input geometry.

pub mod dxf;
pub mod eps;
mod paths;
pub mod pdf;
pub mod png;
pub mod svg;

use av_core::{CoreError, VectorDoc};

/// Supported offline export formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Svg,
    Pdf,
    Eps,
    Dxf,
    Png,
}
impl Format {
    /// Resolve a case-insensitive filename extension or explicit format name.
    pub fn parse(name: &str) -> Result<Self, CoreError> {
        match name.to_ascii_lowercase().as_str() {
            "svg" => Ok(Self::Svg),
            "pdf" => Ok(Self::Pdf),
            "eps" => Ok(Self::Eps),
            "dxf" => Ok(Self::Dxf),
            "png" => Ok(Self::Png),
            _ => Err(CoreError::InvalidParams(format!(
                "unsupported export format: {name}"
            ))),
        }
    }
}

/// Serialize a document without filesystem I/O or mutation of its shared edges.
pub fn export(
    doc: &VectorDoc,
    format: Format,
    options: &svg::SvgOptions,
) -> Result<Vec<u8>, CoreError> {
    paths::validate(doc)?;
    options.validate()?;
    match format {
        Format::Svg => Ok(svg::to_string_with_options(doc, options)?.into_bytes()),
        Format::Pdf => pdf::to_bytes(doc, options),
        Format::Eps => Ok(eps::to_string(doc, options)?.into_bytes()),
        Format::Dxf => Ok(dxf::to_string(doc)?.into_bytes()),
        Format::Png => png::to_bytes(doc, options),
    }
}

/// Counts in the emitted representation, including omitted holes and grouping.
pub fn statistics(
    doc: &VectorDoc,
    format: Format,
    options: &svg::SvgOptions,
) -> Result<av_core::assemble::DocStats, CoreError> {
    paths::validate(doc)?;
    options.validate()?;
    if format == Format::Dxf {
        let used: std::collections::BTreeSet<_> = doc
            .shapes
            .iter()
            .flat_map(|s| s.loops.iter().flatten().map(|&(id, _)| id))
            .collect();
        let mut nodes = 0;
        for id in &used {
            let edge = doc
                .edges
                .get(*id as usize)
                .ok_or_else(|| CoreError::InvalidParams("DXF edge reference".into()))?;
            let points = dxf::flatten_edge(edge)?;
            nodes += points.len() - usize::from(points.first() == points.last());
        }
        return Ok(av_core::assemble::DocStats {
            paths: used.len(),
            nodes,
            colors: 0,
        });
    }
    let paint = paths::paint(doc, options.overlap)?;
    let colors: std::collections::BTreeSet<_> = paint.iter().map(|p| p.rgb).collect();
    let count = if options.group_by_color && matches!(format, Format::Svg | Format::Png) {
        colors.len()
    } else {
        paint.len()
    };
    Ok(av_core::assemble::DocStats {
        paths: count,
        nodes: paint
            .iter()
            .flat_map(|p| &p.paths)
            .map(|p| p.segs.len())
            .sum(),
        colors: colors.len(),
    })
}
