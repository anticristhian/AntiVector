//! Provisional F2 SVG 1.1 writer with exact shared paths and even-odd holes.

use av_core::{
    assemble::{loop_geometry, Seg, VectorDoc},
    CoreError,
};
use std::fmt::Write;

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
    let width = number(doc.width)?;
    let height = number(doc.height)?;
    let mut output = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n");
    for shape in &doc.shapes {
        let color = doc
            .palette
            .colors
            .get(shape.color as usize)
            .ok_or_else(|| CoreError::InvalidParams("SVG palette reference".into()))?;
        let mut d = String::new();
        for path in &shape.loops {
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
