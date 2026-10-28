//! Deterministic weighted 5-5-5 histogram k-means++ (ADR-007, still a hypothesis).

use super::{
    oklab::{self, Oklab},
    LabelMap, Palette, PaletteColor, TRANSPARENT,
};
use crate::{ColorCount, Control, CoreError, Mode, Params, RgbaImage, Stage};
use rand_chacha::{
    rand_core::{RngCore, SeedableRng},
    ChaCha8Rng,
};
use rayon::prelude::*;
#[path = "quantize_alternatives.rs"]
mod alternatives;

#[derive(Clone, Copy, Default)]
struct Bin {
    count: u32,
    sum: [f64; 3],
}

fn nearest(c: Oklab, centers: &[Oklab]) -> usize {
    centers
        .iter()
        .enumerate()
        .min_by(|(ia, a), (ib, b)| {
            c.distance_squared(**a)
                .total_cmp(&c.distance_squared(**b))
                .then(ia.cmp(ib))
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn fit(
    bins: &[(Oklab, u32)],
    positions: &[([usize; 3], Oklab, u32)],
    k: usize,
    params: &Params,
    ctl: &Control,
) -> Result<Vec<Oklab>, CoreError> {
    let mut rng = ChaCha8Rng::seed_from_u64(params.seed);
    let mut centers: Vec<_> = params
        .palette_lock
        .iter()
        .map(|&c| oklab::from_srgb8(c))
        .collect();
    use crate::params::Quantizer;
    let variant = params.experimental.quantizer;
    if variant != Quantizer::Kmeans {
        let initial = match variant {
            Quantizer::MedianCut => alternatives::median_cut(bins, k),
            Quantizer::Wu => alternatives::wu(positions, k),
            Quantizer::Kmeans => unreachable!(),
        };
        for c in initial {
            if centers.len() >= k {
                break;
            }
            if !centers.iter().any(|&p| c.distance_squared(p) < 1e-20) {
                centers.push(c);
            }
        }
    }
    while centers.len() < k {
        ctl.check()?;
        let weights: Vec<_> = bins
            .iter()
            .map(|&(c, n)| {
                f64::from(n)
                    * if centers.is_empty() {
                        1.0
                    } else {
                        c.distance_squared(centers[nearest(c, &centers)])
                    }
            })
            .collect();
        let sum: f64 = weights.iter().sum();
        if sum <= 1e-20 {
            break;
        }
        let target = (rng.next_u64() >> 11) as f64 / ((1_u64 << 53) as f64) * sum;
        let mut cumulative = 0.0;
        let index = weights
            .iter()
            .position(|weight| {
                cumulative += weight;
                cumulative > target
            })
            .unwrap_or(weights.len() - 1);
        centers.push(bins[index].0);
    }
    let iterations = match variant {
        Quantizer::Kmeans => 30,
        Quantizer::MedianCut => 3,
        Quantizer::Wu => 0,
    };
    for _ in 0..iterations {
        ctl.check()?;
        let mut sums = vec![[0.0; 4]; centers.len()];
        for &(c, n) in bins {
            let sum = &mut sums[nearest(c, &centers)];
            let n = f64::from(n);
            sum[0] += c.l * n;
            sum[1] += c.a * n;
            sum[2] += c.b * n;
            sum[3] += n;
        }
        let mut movement: f64 = 0.0;
        for i in params.palette_lock.len()..centers.len() {
            if sums[i][3] > 0.0 {
                let c = Oklab {
                    l: sums[i][0] / sums[i][3],
                    a: sums[i][1] / sums[i][3],
                    b: sums[i][2] / sums[i][3],
                };
                movement = movement.max(centers[i].distance_squared(c));
                centers[i] = c;
            }
        }
        if movement < 1e-8 {
            break;
        }
    }
    // Preserve forced colors even when two locks are perceptually close.
    let mut merged = Vec::new();
    for (i, &c) in centers.iter().enumerate() {
        if i < params.palette_lock.len()
            || !merged
                .iter()
                .any(|&m| c.distance_squared(m) < 0.015_f64.powi(2))
        {
            merged.push(c);
        }
    }
    Ok(merged)
}

/// Quantize pixels; ordering and all floating-point reductions are independent of threads.
#[tracing::instrument(skip_all)]
pub fn quantize(
    image: &RgbaImage,
    params: &Params,
    ctl: &Control,
) -> Result<(Palette, LabelMap), CoreError> {
    image.validate(crate::raster::DEFAULT_PIXEL_LIMIT)?;
    params.validate()?;
    ctl.report(Stage::Quantize, 0.0)?;
    let mut histogram = vec![Bin::default(); 32768];
    for (i, pixel) in image.px.iter().enumerate() {
        if i % (image.w as usize * 64) == 0 {
            ctl.check()?;
        }
        if pixel[3] < params.transparency_alpha_cut {
            continue;
        }
        let [r, g, b, _] = *pixel;
        let index = ((r as usize >> 3) << 10) | ((g as usize >> 3) << 5) | (b as usize >> 3);
        let c = oklab::from_srgb8([r, g, b]);
        let bin = &mut histogram[index];
        bin.count += 1;
        bin.sum[0] += c.l;
        bin.sum[1] += c.a;
        bin.sum[2] += c.b;
    }
    let positions: Vec<_> = histogram
        .iter()
        .enumerate()
        .filter(|(_, b)| b.count > 0)
        .map(|(index, b)| {
            let n = f64::from(b.count);
            (
                [(index >> 10) & 31, (index >> 5) & 31, index & 31],
                Oklab {
                    l: b.sum[0] / n,
                    a: b.sum[1] / n,
                    b: b.sum[2] / n,
                },
                b.count,
            )
        })
        .collect();
    let bins: Vec<_> = histogram
        .iter()
        .filter(|b| b.count > 0)
        .map(|b| {
            let n = f64::from(b.count);
            (
                Oklab {
                    l: b.sum[0] / n,
                    a: b.sum[1] / n,
                    b: b.sum[2] / n,
                },
                b.count,
            )
        })
        .collect();
    let mut labels = LabelMap {
        w: image.w,
        h: image.h,
        labels: vec![TRANSPARENT; image.px.len()],
    };
    if bins.is_empty() {
        let palette = Palette {
            colors: params
                .palette_lock
                .iter()
                .enumerate()
                .map(|(i, &rgb)| PaletteColor {
                    id: i as u16,
                    rgb,
                    oklab: oklab::from_srgb8(rgb),
                    locked: true,
                })
                .collect(),
        };
        ctl.report(Stage::Quantize, 1.0)?;
        return Ok((palette, labels));
    }
    let mut centers = match params.colors {
        ColorCount::Fixed(k) => fit(
            &bins,
            &positions,
            usize::from(k).max(params.palette_lock.len()),
            params,
            ctl,
        )?,
        ColorCount::Auto => {
            let (limit, tolerance) = match params.mode {
                Mode::Logo | Mode::LineArt => (64, 0.02),
                Mode::Illustration => (64, 0.03),
                Mode::Photo => (128, 0.05),
            };
            let mut result = Vec::new();
            let minimum = if params.experimental.quality_v5 {
                if params.mode == Mode::Photo {
                    24
                } else {
                    crate::detect(image).colors as usize
                }
            } else {
                2
            };
            for k in minimum.max(2).max(params.palette_lock.len()).min(limit)..=limit {
                result = fit(&bins, &positions, k, params, ctl)?;
                let error = bins
                    .iter()
                    .map(|&(c, n)| {
                        c.distance_squared(result[nearest(c, &result)]).sqrt() * f64::from(n)
                    })
                    .sum::<f64>()
                    / bins.iter().map(|b| f64::from(b.1)).sum::<f64>();
                if error <= tolerance || result.len() >= bins.len() {
                    break;
                }
            }
            result
        }
    };
    ctl.report(Stage::Quantize, 0.6)?;
    labels
        .labels
        .par_chunks_mut(image.w as usize)
        .enumerate()
        .try_for_each(|(row, output)| -> Result<(), CoreError> {
            ctl.check()?;
            for (x, label) in output.iter_mut().enumerate() {
                let pixel = image.px[row * image.w as usize + x];
                if pixel[3] >= params.transparency_alpha_cut {
                    *label =
                        nearest(oklab::from_srgb8([pixel[0], pixel[1], pixel[2]]), &centers) as u16;
                }
            }
            Ok(())
        })?;
    let mut sums = vec![[0.0; 4]; centers.len()];
    let mut candidate_rgb = vec![[0; 3]; centers.len()];
    let mut votes = vec![0_u32; centers.len()];
    for (i, (&label, pixel)) in labels.labels.iter().zip(&image.px).enumerate() {
        if i % (image.w as usize * 64) == 0 {
            ctl.check()?;
        }
        if label == TRANSPARENT {
            continue;
        }
        let sum = &mut sums[label as usize];
        for channel in 0..3 {
            sum[channel] += oklab::linear_lut()[pixel[channel] as usize];
        }
        sum[3] += 1.0;
        if params.experimental.quality_v5 {
            let index = label as usize;
            let rgb = [pixel[0], pixel[1], pixel[2]];
            if votes[index] == 0 {
                candidate_rgb[index] = rgb;
                votes[index] = 1;
            } else if candidate_rgb[index] == rgb {
                votes[index] += 1;
            } else {
                votes[index] -= 1;
            }
        }
    }
    let mut exact_counts = vec![0_u32; centers.len()];
    if params.experimental.quality_v5 {
        for (&label, pixel) in labels.labels.iter().zip(&image.px) {
            if label != TRANSPARENT
                && candidate_rgb[label as usize] == [pixel[0], pixel[1], pixel[2]]
            {
                exact_counts[label as usize] += 1;
            }
        }
    }
    let colors: Vec<_> = centers
        .iter_mut()
        .enumerate()
        .map(|(i, center)| {
            let locked = i < params.palette_lock.len();
            let rgb = if locked {
                params.palette_lock[i]
            } else if params.experimental.quality_v5
                && f64::from(exact_counts[i]) >= 0.6 * sums[i][3]
                && sums[i][3] > 0.0
            {
                candidate_rgb[i]
            } else if sums[i][3] > 0.0 {
                std::array::from_fn(|ch| {
                    (oklab::linear_to_srgb(sums[i][ch] / sums[i][3]).clamp(0.0, 1.0) * 255.0)
                        .round() as u8
                })
            } else {
                oklab::to_srgb8(*center)
            };
            PaletteColor {
                id: i as u16,
                rgb,
                oklab: oklab::from_srgb8(rgb),
                locked,
            }
        })
        .collect();
    // Palette IDs, like region IDs, follow first raster occurrence. Unused forced
    // colors retain their stable relative order after the colors seen in pixels.
    let mut remap = vec![TRANSPARENT; colors.len()];
    let mut ordered = Vec::with_capacity(colors.len());
    for old in labels
        .labels
        .iter_mut()
        .filter(|label| **label != TRANSPARENT)
    {
        if remap[*old as usize] == TRANSPARENT {
            let mut color = colors[*old as usize].clone();
            color.id = ordered.len() as u16;
            remap[*old as usize] = color.id;
            ordered.push(color);
        }
        *old = remap[*old as usize];
    }
    for (i, mut color) in colors.into_iter().enumerate() {
        if remap[i] == TRANSPARENT {
            color.id = ordered.len() as u16;
            ordered.push(color);
        }
    }
    ctl.report(Stage::Quantize, 1.0)?;
    Ok((Palette { colors: ordered }, labels))
}
