//! SVG 1.1 writer with shared paths and safe opaque overlap (ADR-010).

use av_core::{
    assemble::{loop_geometry, Seg, VectorDoc},
    CoreError,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

/// Safe later-painted opaque hole omission by default; explicit exact mode.
#[derive(Clone, Copy, Debug)]
pub struct SvgOptions {
    pub overlap: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self { overlap: true }
    }
}

fn omit_hole(doc: &VectorDoc, index: usize, hole: usize, regions: &BTreeMap<u32, usize>) -> bool {
    let Some(contents) = doc.shapes[index].hole_regions.get(hole) else {
        return false;
    };
    if contents.is_empty() {
        return false;
    }
    let mut pending = contents.clone();
    let mut visited = BTreeSet::new();
    while let Some(region) = pending.pop() {
        if !visited.insert(region) {
            continue;
        }
        let Some(&other) = regions.get(&region) else {
            return false;
        };
        if other <= index {
            return false;
        }
        pending.extend(doc.shapes[other].hole_regions.iter().flatten().copied());
    }
    true
}

fn number(value: f64) -> Result<String, CoreError> {
    if !value.is_finite() {
        return Err(CoreError::InvalidParams("non-finite SVG coordinate".into()));
    }
    let value = if value == 0.0 { 0.0 } else { value };
    let mut text = format!("{value:.2}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    Ok(text)
}

/// Serialize deterministically; path coordinates use the identical shared edge data.
pub fn to_string(doc: &VectorDoc) -> Result<String, CoreError> {
    to_string_with_options(doc, &SvgOptions::default())
}

/// Omit only holes whose complete opaque contents are painted later. Exact mode
/// retains all loops; both modes preserve the canonical shared curve geometry.
pub fn to_string_with_options(doc: &VectorDoc, options: &SvgOptions) -> Result<String, CoreError> {
    let width = number(doc.width)?;
    let height = number(doc.height)?;
    let mut output = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n");
    let regions: BTreeMap<_, _> = doc
        .shapes
        .iter()
        .enumerate()
        .map(|(i, s)| (s.region, i))
        .collect();
    for (index, shape) in doc.shapes.iter().enumerate() {
        let color = doc
            .palette
            .colors
            .get(shape.color as usize)
            .ok_or_else(|| CoreError::InvalidParams("SVG palette reference".into()))?;
        let mut d = String::new();
        for (loop_index, path) in shape.loops.iter().enumerate() {
            if options.overlap && loop_index > 0 && omit_hole(doc, index, loop_index - 1, &regions)
            {
                continue;
            }
            let geometry = loop_geometry(&doc.edges, path)?;
            write!(
                d,
                "M{} {}",
                number(geometry.start[0])?,
                number(geometry.start[1])?
            )
            .unwrap();
            for seg in geometry.segs {
                match seg {
                    Seg::Line(p) => write!(d, "L{} {}", number(p[0])?, number(p[1])?).unwrap(),
                    Seg::Cubic(a, b, p) => write!(
                        d,
                        "C{} {} {} {} {} {}",
                        number(a[0])?,
                        number(a[1])?,
                        number(b[0])?,
                        number(b[1])?,
                        number(p[0])?,
                        number(p[1])?
                    )
                    .unwrap(),
                }
            }
            d.push('Z');
        }
        writeln!(
            output,
            "<path fill=\"#{:02x}{:02x}{:02x}\" fill-rule=\"evenodd\" d=\"{}\"/>",
            color.rgb[0], color.rgb[1], color.rgb[2], d
        )
        .unwrap();
    }
    output.push_str("</svg>\n");
    Ok(output)
}
