//! SVG 1.1 writer with shared paths and safe opaque overlap (ADR-010).

use av_core::{
    assemble::{Seg, VectorDoc},
    CoreError,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

/// Safe later-painted opaque hole omission by default; explicit exact mode.
#[derive(Clone, Copy, Debug)]
pub struct SvgOptions {
    pub overlap: bool,
    /// Decimal places, 0..=8. Two preserves the validated F5 default bytes.
    pub precision: u8,
    /// Merge paths by RGB. Uses exact holes; overlap and grouping are exclusive.
    pub group_by_color: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            overlap: true,
            precision: 2,
            group_by_color: false,
        }
    }
}

pub(crate) fn omit_hole(
    doc: &VectorDoc,
    index: usize,
    hole: usize,
    regions: &BTreeMap<u32, usize>,
) -> bool {
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

fn number(value: f64, precision: u8) -> Result<String, CoreError> {
    if !value.is_finite() {
        return Err(CoreError::InvalidParams("non-finite SVG coordinate".into()));
    }
    let value = if value == 0.0 { 0.0 } else { value };
    let mut text = format!("{value:.precision$}", precision = usize::from(precision));
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text == "-0" {
        text = "0".into();
    }
    Ok(text)
}

impl SvgOptions {
    /// Validate precision and the paint-order constraint for grouped paths.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.precision > 8 {
            return Err(CoreError::InvalidParams("precision must be 0..=8".into()));
        }
        if self.group_by_color && self.overlap {
            return Err(CoreError::InvalidParams(
                "color grouping requires exact stacking".into(),
            ));
        }
        Ok(())
    }
}

/// Serialize deterministically; path coordinates use the identical shared edge data.
pub fn to_string(doc: &VectorDoc) -> Result<String, CoreError> {
    to_string_with_options(doc, &SvgOptions::default())
}

/// Omit only holes whose complete opaque contents are painted later. Exact mode
/// retains all loops; both modes preserve the canonical shared curve geometry.
pub fn to_string_with_options(doc: &VectorDoc, options: &SvgOptions) -> Result<String, CoreError> {
    options.validate()?;
    let number = |v| number(v, options.precision);
    let width = number(doc.width)?;
    let height = number(doc.height)?;
    let mut output = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n");
    let mut paints = crate::paths::paint(doc, options.overlap)?;
    if options.group_by_color {
        let mut groups: BTreeMap<[u8; 3], Vec<_>> = BTreeMap::new();
        for paint in paints {
            groups.entry(paint.rgb).or_default().extend(paint.paths);
        }
        paints = groups
            .into_iter()
            .map(|(rgb, paths)| crate::paths::Paint { rgb, paths })
            .collect();
    }
    for paint in paints {
        let mut d = String::new();
        for geometry in paint.paths {
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
            paint.rgb[0], paint.rgb[1], paint.rgb[2], d
        )
        .unwrap();
    }
    output.push_str("</svg>\n");
    Ok(output)
}
