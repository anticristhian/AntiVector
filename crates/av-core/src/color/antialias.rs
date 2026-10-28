//! Relabel two-color transition pixels without creating independent halo regions.

use super::{
    compact_palette,
    oklab::{self, Oklab},
    LabelMap, Palette, TRANSPARENT,
};
use crate::{Control, CoreError, Mode, Params, RgbaImage, Stage};
use rayon::prelude::*;

/// Resolved labels and the pixels identified as mixed-color transition candidates.
pub struct AaResult {
    pub palette: Palette,
    pub labels: LabelMap,
    pub mixture_pixels: Vec<bool>,
}

/// Project color onto candidate neighbor-center segments; ties are stable by ID.
#[tracing::instrument(skip_all)]
pub fn resolve_aa(
    image: &RgbaImage,
    palette: &Palette,
    labels: &LabelMap,
    p: &Params,
    ctl: &Control,
) -> Result<AaResult, CoreError> {
    image.validate(crate::raster::DEFAULT_PIXEL_LIMIT)?;
    labels.validate()?;
    p.validate()?;
    if (image.w, image.h) != (labels.w, labels.h)
        || labels
            .labels
            .iter()
            .any(|&id| id != TRANSPARENT && id as usize >= palette.colors.len())
    {
        return Err(CoreError::InvalidParams("AA image/palette mismatch".into()));
    }
    ctl.report(Stage::ResolveAa, 0.0)?;
    let mut labels = labels.clone();
    let mut mixture_pixels = vec![false; labels.labels.len()];
    if p.antialias_resolve {
        let source: Vec<_> = image
            .px
            .par_iter()
            .map(|pixel| oklab::from_srgb8([pixel[0], pixel[1], pixel[2]]))
            .collect();
        let (passes, tolerance) = if p.mode == Mode::Photo {
            (1, 0.02_f64)
        } else {
            (2, 0.03_f64)
        };
        let w = image.w as usize;
        let h = image.h as usize;
        for pass in 0..passes {
            let resolved: Vec<_> = labels
                .labels
                .par_iter()
                .enumerate()
                .map(|(i, &current)| -> Result<(u16, bool), CoreError> {
                    if i % 1024 == 0 {
                        ctl.check()?;
                    }
                    if current == TRANSPARENT {
                        return Ok((current, false));
                    }
                    let x = i % w;
                    let y = i / w;
                    let mut neighbors = [TRANSPARENT; 9];
                    let mut count = 0;
                    for sy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                        for sx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                            let id = labels.labels[sy * w + sx];
                            if id != TRANSPARENT && !neighbors[..count].contains(&id) {
                                neighbors[count] = id;
                                count += 1;
                            }
                        }
                    }
                    neighbors[..count].sort_unstable();
                    let mut best: Option<(f64, u16, u16, u16)> = None;
                    for ai in 0..count {
                        for bi in ai + 1..count {
                            let a = neighbors[ai];
                            let b = neighbors[bi];
                            if current == a || current == b {
                                continue;
                            }
                            let ca = palette.colors[a as usize].oklab;
                            let cb = palette.colors[b as usize].oklab;
                            let color = source[i];
                            let v = [cb.l - ca.l, cb.a - ca.a, cb.b - ca.b];
                            let q = [color.l - ca.l, color.a - ca.a, color.b - ca.b];
                            let denominator = v.iter().map(|v| v * v).sum::<f64>();
                            if denominator < 1e-20 {
                                continue;
                            }
                            let t = q.iter().zip(v).map(|(q, v)| q * v).sum::<f64>() / denominator;
                            if !(0.1..0.9).contains(&t) {
                                continue;
                            }
                            let projected = Oklab {
                                l: ca.l + t * v[0],
                                a: ca.a + t * v[1],
                                b: ca.b + t * v[2],
                            };
                            let distance = color.distance_squared(projected);
                            if distance < tolerance * tolerance
                                && best.as_ref().is_none_or(|&(d, ba, bb, _)| {
                                    distance.total_cmp(&d).then((a, b).cmp(&(ba, bb))).is_lt()
                                })
                            {
                                best = Some((distance, a, b, if t < 0.5 { a } else { b }));
                            }
                        }
                    }
                    Ok(best.map_or((current, false), |(_, _, _, label)| (label, true)))
                })
                .collect::<Result<_, _>>()?;
            for (i, (label, mixture)) in resolved.into_iter().enumerate() {
                labels.labels[i] = label;
                mixture_pixels[i] |= mixture;
            }
            ctl.report(Stage::ResolveAa, (pass + 1) as f32 / passes as f32)?;
        }
    }
    let palette = compact_palette(palette, &mut labels)?;
    ctl.report(Stage::ResolveAa, 1.0)?;
    Ok(AaResult {
        palette,
        labels,
        mixture_pixels,
    })
}
