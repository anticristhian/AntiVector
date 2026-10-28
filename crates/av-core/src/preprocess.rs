//! Alpha cleanup, deterministic bicubic enlargement and edge-preserving denoising.
//! Guided-filter alternative follows the local linear model in He et al. (2010):
//! <https://people.csail.mit.edu/kaiming/publications/eccv10guidedfilter.pdf>.

use crate::{
    color::oklab::{self, Oklab},
    detect::Detection,
    params::DenoiseKind,
    Control, CoreError, Params, RgbaImage, Stage,
};
use rayon::prelude::*;

/// Prepared raster and its integer coordinate scale relative to the input.
pub struct PreparedRaster {
    pub image: RgbaImage,
    pub scale: u32,
}

fn alpha_cleanup(image: &RgbaImage, cut: u8, ctl: &Control) -> Result<RgbaImage, CoreError> {
    let mut pixels = image.px.clone();
    let w = image.w as usize;
    let h = image.h as usize;
    pixels
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(i, output)| -> Result<(), CoreError> {
            if i % 1024 == 0 {
                ctl.check()?;
            }
            let original = image.px[i];
            if original[3] < cut {
                *output = [0; 4];
                return Ok(());
            }
            if original[3] == 255 {
                return Ok(());
            }
            let x = i % w;
            let y = i / w;
            let mut background = [0.0; 3];
            let mut count = 0;
            for sy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for sx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    let pixel = image.px[sy * w + sx];
                    if pixel[3] == 255 {
                        for ch in 0..3 {
                            background[ch] += oklab::linear_lut()[pixel[ch] as usize];
                        }
                        count += 1;
                    }
                }
            }
            let alpha = f64::from(original[3]) / 255.0;
            for ch in 0..3 {
                let source = oklab::linear_lut()[original[ch] as usize];
                let behind = if count > 0 {
                    background[ch] / f64::from(count)
                } else {
                    source
                };
                output[ch] = (oklab::linear_to_srgb(source * alpha + behind * (1.0 - alpha))
                    .clamp(0.0, 1.0)
                    * 255.0)
                    .round() as u8;
            }
            output[3] = 255;
            Ok(())
        })?;
    RgbaImage::new(image.w, image.h, pixels)
}

fn cubic_weight(v: f64) -> f64 {
    let x = v.abs();
    if x <= 1.0 {
        1.5 * x.powi(3) - 2.5 * x * x + 1.0
    } else if x < 2.0 {
        -0.5 * x.powi(3) + 2.5 * x * x - 4.0 * x + 2.0
    } else {
        0.0
    }
}

fn upscale(image: &RgbaImage, cut: u8, ctl: &Control) -> Result<RgbaImage, CoreError> {
    let w = image.w * 2;
    let h = image.h * 2;
    let count = crate::raster::validate_dimensions(w, h, crate::raster::DEFAULT_PIXEL_LIMIT)?;
    let mut output = vec![[0; 4]; count];
    output
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(i, pixel)| -> Result<(), CoreError> {
            if i % 1024 == 0 {
                ctl.check()?;
            }
            let x = (i % w as usize) as f64 / 2.0 - 0.25;
            let y = (i / w as usize) as f64 / 2.0 - 0.25;
            let bx = x.floor() as i64;
            let by = y.floor() as i64;
            let mut sum = [0.0; 4];
            for dy in -1..=2 {
                for dx in -1..=2 {
                    let sx = (bx + dx).clamp(0, i64::from(image.w) - 1) as usize;
                    let sy = (by + dy).clamp(0, i64::from(image.h) - 1) as usize;
                    let weight =
                        cubic_weight(x - (bx + dx) as f64) * cubic_weight(y - (by + dy) as f64);
                    let source = image.px[sy * image.w as usize + sx];
                    let alpha = f64::from(source[3]) / 255.0;
                    for ch in 0..3 {
                        sum[ch] += weight * alpha * f64::from(source[ch]);
                    }
                    sum[3] += weight * alpha;
                }
            }
            if sum[3].clamp(0.0, 1.0) * 255.0 >= f64::from(cut) {
                for ch in 0..3 {
                    pixel[ch] = (sum[ch] / sum[3]).clamp(0.0, 255.0).round() as u8;
                }
                pixel[3] = 255;
            }
            Ok(())
        })?;
    RgbaImage::new(w, h, output)
}

fn neighbors(
    image: &RgbaImage,
    x: usize,
    y: usize,
    radius: usize,
    mut visit: impl FnMut(usize, usize, usize),
) {
    for sy in y.saturating_sub(radius)..=(y + radius).min(image.h as usize - 1) {
        for sx in x.saturating_sub(radius)..=(x + radius).min(image.w as usize - 1) {
            let i = sy * image.w as usize + sx;
            if image.px[i][3] > 0 {
                visit(i, sx, sy);
            }
        }
    }
}

fn bilateral(
    image: &RgbaImage,
    lab: &[Oklab],
    radius: usize,
    sigma: f64,
    mean_shift: bool,
    ctl: &Control,
) -> Result<Vec<Oklab>, CoreError> {
    let mut output = lab.to_vec();
    output
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(i, result)| -> Result<(), CoreError> {
            if i % 1024 == 0 {
                ctl.check()?;
            }
            if image.px[i][3] == 0 {
                return Ok(());
            }
            let mut center = lab[i];
            let mut cx = (i % image.w as usize) as f64;
            let mut cy = (i / image.w as usize) as f64;
            for _ in 0..if mean_shift { 5 } else { 1 } {
                let mut sum = [0.0; 6];
                neighbors(
                    image,
                    cx.round() as usize,
                    cy.round() as usize,
                    radius,
                    |j, x, y| {
                        let spatial = (x as f64 - cx).powi(2) + (y as f64 - cy).powi(2);
                        let weight = (-spatial / (2.0 * (radius as f64).powi(2))
                            - lab[j].distance_squared(center) / (2.0 * sigma * sigma))
                            .exp();
                        sum[0] += weight * lab[j].l;
                        sum[1] += weight * lab[j].a;
                        sum[2] += weight * lab[j].b;
                        sum[3] += weight * x as f64;
                        sum[4] += weight * y as f64;
                        sum[5] += weight;
                    },
                );
                if sum[5] <= 0.0 {
                    break;
                }
                let next = Oklab {
                    l: sum[0] / sum[5],
                    a: sum[1] / sum[5],
                    b: sum[2] / sum[5],
                };
                let nx = sum[3] / sum[5];
                let ny = sum[4] / sum[5];
                let converged =
                    next.distance_squared(center) < 1e-8 && (nx - cx).hypot(ny - cy) < 0.01;
                center = next;
                cx = nx.clamp(0.0, f64::from(image.w - 1));
                cy = ny.clamp(0.0, f64::from(image.h - 1));
                if converged {
                    break;
                }
            }
            *result = center;
            Ok(())
        })?;
    Ok(output)
}

fn guided(
    image: &RgbaImage,
    lab: &[Oklab],
    radius: usize,
    epsilon: f64,
    ctl: &Control,
) -> Result<Vec<Oklab>, CoreError> {
    let mut coefficients = vec![[0.0; 6]; lab.len()];
    coefficients.par_iter_mut().enumerate().try_for_each(
        |(i, coeff)| -> Result<(), CoreError> {
            if i % 1024 == 0 {
                ctl.check()?;
            }
            if image.px[i][3] == 0 {
                return Ok(());
            }
            let mut sum = [0.0; 9];
            neighbors(
                image,
                i % image.w as usize,
                i / image.w as usize,
                radius,
                |j, _, _| {
                    let c = lab[j];
                    let guide = c.l;
                    let v = [c.l, c.a, c.b];
                    sum[0] += guide;
                    sum[1] += guide * guide;
                    sum[8] += 1.0;
                    for ch in 0..3 {
                        sum[2 + ch] += v[ch];
                        sum[5 + ch] += guide * v[ch];
                    }
                },
            );
            let mean = sum[0] / sum[8];
            let variance = (sum[1] / sum[8] - mean * mean).max(0.0);
            for ch in 0..3 {
                coeff[ch] =
                    (sum[5 + ch] / sum[8] - mean * sum[2 + ch] / sum[8]) / (variance + epsilon);
                coeff[3 + ch] = sum[2 + ch] / sum[8] - coeff[ch] * mean;
            }
            Ok(())
        },
    )?;
    let mut output = lab.to_vec();
    output
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(i, result)| -> Result<(), CoreError> {
            if i % 1024 == 0 {
                ctl.check()?;
            }
            if image.px[i][3] == 0 {
                return Ok(());
            }
            let mut sum = [0.0; 3];
            let mut count = 0;
            neighbors(
                image,
                i % image.w as usize,
                i / image.w as usize,
                radius,
                |j, _, _| {
                    for (ch, value) in sum.iter_mut().enumerate() {
                        *value += coefficients[j][ch] * lab[i].l + coefficients[j][ch + 3];
                    }
                    count += 1;
                },
            );
            *result = Oklab {
                l: sum[0] / f64::from(count),
                a: sum[1] / f64::from(count),
                b: sum[2] / f64::from(count),
            };
            Ok(())
        })?;
    Ok(output)
}

/// Prepare validated pixels. Algorithms are deterministic across Rayon thread counts.
#[tracing::instrument(skip_all)]
pub fn preprocess(
    image: &RgbaImage,
    params: &Params,
    detection: &Detection,
    ctl: &Control,
) -> Result<PreparedRaster, CoreError> {
    image.validate(crate::raster::DEFAULT_PIXEL_LIMIT)?;
    params.validate()?;
    ctl.report(Stage::Preprocess, 0.0)?;
    let pixel_art = detection.is_pixel_art || params.experimental.force_pixel_art;
    let mut prepared = alpha_cleanup(image, params.transparency_alpha_cut, ctl)?;
    // Upsampling a multicolor frame introduces short boundary fragments where
    // the graph must keep frame junctions fixed. Retain original sampling there.
    let first = prepared.px[0];
    let uniform_frame = prepared
        .px
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let x = *i % image.w as usize;
            let y = *i / image.w as usize;
            x == 0 || y == 0 || x + 1 == image.w as usize || y + 1 == image.h as usize
        })
        .all(|(_, pixel)| *pixel == first);
    let scale = if params.experimental.upscale_small
        && image.w.max(image.h) < 256
        && !pixel_art
        && uniform_frame
    {
        prepared = upscale(&prepared, params.transparency_alpha_cut, ctl)?;
        2
    } else {
        1
    };
    let amount = if pixel_art {
        0.0
    } else {
        f64::from(params.denoise)
    };
    if amount > 0.0 {
        let lab: Vec<_> = prepared
            .px
            .par_iter()
            .map(|p| oklab::from_srgb8([p[0], p[1], p[2]]))
            .collect();
        let radius = (1.0 + 2.0 * amount).round() as usize;
        let sigma = 0.02 + 0.08 * amount;
        let filtered = match params.experimental.denoise_kind {
            DenoiseKind::Bilateral => bilateral(&prepared, &lab, radius, sigma, false, ctl)?,
            DenoiseKind::MeanShift => bilateral(&prepared, &lab, radius, sigma, true, ctl)?,
            DenoiseKind::Guided => guided(&prepared, &lab, radius, sigma * sigma, ctl)?,
        };
        prepared
            .px
            .par_iter_mut()
            .zip(filtered)
            .for_each(|(pixel, c)| {
                if pixel[3] > 0 {
                    let rgb = oklab::to_srgb8(c);
                    pixel[..3].copy_from_slice(&rgb);
                }
            });
    }
    ctl.report(Stage::Preprocess, 1.0)?;
    Ok(PreparedRaster {
        image: prepared,
        scale,
    })
}
