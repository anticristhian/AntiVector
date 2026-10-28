//! Detection data contract; detector calibration belongs to F3.

use crate::Mode;

/// Image observations consumed by mode defaults and future automatic detection.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Detection {
    pub mode: Mode,
    pub colors: u16,
    pub has_alpha: bool,
    pub is_jpeg_like: bool,
    pub is_pixel_art: bool,
    pub width: u32,
    pub height: u32,
}

/// Initial documented detector. Thresholds remain hypotheses until the F3 corpus run.
/// Malformed/empty rasters return a conservative suggestion; vectorize rejects them.
#[tracing::instrument(skip_all)]
pub fn detect(image: &crate::RgbaImage) -> Detection {
    let mut result = Detection {
        mode: Mode::Logo,
        colors: 2,
        has_alpha: image.has_alpha(),
        is_jpeg_like: false,
        is_pixel_art: false,
        width: image.w,
        height: image.h,
    };
    if image.validate(crate::raster::DEFAULT_PIXEL_LIMIT).is_err() {
        return result;
    }
    let scale = (f64::from(image.w.max(image.h)) / 512.0).max(1.0);
    let w = (f64::from(image.w) / scale).round().max(1.0) as usize;
    let h = (f64::from(image.h) / scale).round().max(1.0) as usize;
    // Integer box-filter footprints; detection uses <=512px on the largest side.
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let x0 = x * image.w as usize / w;
            let x1 = ((x + 1) * image.w as usize / w).max(x0 + 1);
            let y0 = y * image.h as usize / h;
            let y1 = ((y + 1) * image.h as usize / h).max(y0 + 1);
            let mut sum = [0_u64; 4];
            let mut count = 0;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let p = image.px[sy * image.w as usize + sx];
                    for ch in 0..4 {
                        sum[ch] += u64::from(p[ch]);
                    }
                    count += 1;
                }
            }
            pixels.push(sum.map(|v| (v / count) as u8));
        }
    }
    let mut histogram = std::collections::BTreeMap::<[u8; 3], u32>::new();
    for p in &pixels {
        if p[3] >= 128 {
            *histogram.entry([p[0], p[1], p[2]]).or_default() += 1;
        }
    }
    let unique = histogram.len();
    let mut colors: Vec<_> = histogram.into_iter().collect();
    colors.sort_by_key(|&(rgb, n)| (std::cmp::Reverse(n), rgb));
    let total: u32 = colors.iter().map(|c| c.1).sum();
    let mut clusters = Vec::<(crate::color::oklab::Oklab, u32)>::new();
    // At most 4096 representatives: lower-frequency colors are counted as residual.
    for &(rgb, n) in colors.iter().take(4096) {
        let lab = crate::color::oklab::from_srgb8(rgb);
        if let Some((_, count)) = clusters
            .iter_mut()
            .find(|(c, _)| lab.distance_squared(*c) <= 0.04_f64.powi(2))
        {
            *count += n;
        } else {
            clusters.push((lab, n));
        }
    }
    clusters.sort_by_key(|c| std::cmp::Reverse(c.1));
    let mut cumulative = 0;
    let dominant90 = clusters
        .iter()
        .position(|c| {
            cumulative += c.1;
            u64::from(cumulative) * 10 >= u64::from(total) * 9
        })
        .map_or(usize::MAX, |i| i + 1);
    // The first corpus run lost accent colors occupying <10% of five logos.
    // Retain >=0.5% minority clusters unless explained as a two-color AA mixture.
    let stable = clusters
        .iter()
        .enumerate()
        .filter(|&(i, (c, n))| {
            if u64::from(*n) * 200 < u64::from(total) {
                return false;
            }
            for a in 0..i {
                for b in a + 1..i {
                    let ca = clusters[a].0;
                    let cb = clusters[b].0;
                    let v = [cb.l - ca.l, cb.a - ca.a, cb.b - ca.b];
                    let p = [c.l - ca.l, c.a - ca.a, c.b - ca.b];
                    let denom = v.iter().map(|v| v * v).sum::<f64>();
                    if denom == 0.0 {
                        continue;
                    }
                    let t = p.iter().zip(v).map(|(p, v)| p * v).sum::<f64>() / denom;
                    let residual = p
                        .iter()
                        .zip(v)
                        .map(|(p, v)| (p - t * v).powi(2))
                        .sum::<f64>();
                    if (0.1..0.9).contains(&t) && residual < 0.03_f64.powi(2) {
                        return false;
                    }
                }
            }
            true
        })
        .count();
    let dominant = dominant90.max(stable);
    let lum: Vec<f64> = pixels
        .iter()
        .map(|p| 0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2]))
        .collect();
    let mut edge_count = 0;
    let mut sharp_count = 0;
    let mut energy = 0.0;
    let mut samples = 0;
    let mut block_energy = 0.0;
    let mut block_samples = 0;
    let mut small_gradients = 0;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            for (boundary, neighbor) in [
                (x % 8 == 0, if x > 0 { Some(i - 1) } else { None }),
                (y % 8 == 0, if y > 0 { Some(i - w) } else { None }),
            ] {
                if let Some(j) = neighbor {
                    let d = (lum[i] - lum[j]).abs();
                    let residual = if d <= 20.0 { d } else { 0.0 };
                    energy += residual;
                    if d > 0.5 && d <= 20.0 {
                        small_gradients += 1;
                    }
                    samples += 1;
                    if boundary {
                        block_energy += residual;
                        block_samples += 1;
                    }
                }
            }
            if x == 0 || y == 0 || x + 1 == w || y + 1 == h {
                continue;
            }
            let gx = -lum[i - w - 1] + lum[i - w + 1] - 2.0 * lum[i - 1] + 2.0 * lum[i + 1]
                - lum[i + w - 1]
                + lum[i + w + 1];
            let gy = -lum[i - w - 1] - 2.0 * lum[i - w] - lum[i - w + 1]
                + lum[i + w - 1]
                + 2.0 * lum[i + w]
                + lum[i + w + 1];
            if gx.hypot(gy) > 40.0 {
                edge_count += 1;
                let d = if gx.abs() >= gy.abs() { 1 } else { w };
                let lo = lum[i - d].min(lum[i + d]);
                let hi = lum[i - d].max(lum[i + d]);
                if (lum[i] - lo).abs().min((lum[i] - hi).abs()) < 8.0 {
                    sharp_count += 1;
                }
            }
        }
    }
    let sharp = if edge_count == 0 {
        1.0
    } else {
        sharp_count as f64 / edge_count as f64
    };
    result.is_jpeg_like = energy > 0.0
        && small_gradients > pixels.len() / 10
        && block_samples > 0
        && block_energy / block_samples as f64 / (energy / samples as f64) > 1.3;
    let repeated = if image.w.max(image.h) <= 256 {
        (2..=8).any(|factor: usize| {
            let mut matching = 0;
            let mut boundary_count = 0;
            let mut boundary_matching = 0;
            for y in 0..image.h as usize {
                for x in 0..image.w as usize {
                    let anchor = (y / factor * factor) * image.w as usize + x / factor * factor;
                    if image.px[y * image.w as usize + x] == image.px[anchor] {
                        matching += 1;
                    }
                    let i = y * image.w as usize + x;
                    let boundary = [
                        if x > 0 { Some(i - 1) } else { None },
                        if x + 1 < image.w as usize {
                            Some(i + 1)
                        } else {
                            None
                        },
                        if y > 0 {
                            Some(i - image.w as usize)
                        } else {
                            None
                        },
                        if y + 1 < image.h as usize {
                            Some(i + image.w as usize)
                        } else {
                            None
                        },
                    ]
                    .into_iter()
                    .flatten()
                    .any(|j| image.px[j] != image.px[i]);
                    if boundary {
                        boundary_count += 1;
                        if image.px[i] == image.px[anchor] {
                            boundary_matching += 1;
                        }
                    }
                }
            }
            matching * 100 >= image.px.len() * 95
                && boundary_matching * 100 >= boundary_count * 95
                && image.w as usize >= factor
                && image.h as usize >= factor
        })
    } else {
        false
    };
    result.is_pixel_art = image.w.max(image.h) <= 256
        && (repeated
            || unique <= 2 && sharp > 0.9
            || image.w.max(image.h) <= 64 && unique <= 64 && sharp > 0.9);
    let neutral = pixels
        .iter()
        .filter(|p| p[3] >= 128 && p[..3].iter().max().unwrap() - p[..3].iter().min().unwrap() <= 8)
        .count();
    let mut peaks = [0_u32; 16];
    for (p, &l) in pixels.iter().zip(&lum) {
        if p[3] >= 128 {
            peaks[(l / 16.0).clamp(0.0, 15.0) as usize] += 1;
        }
    }
    peaks.sort_unstable();
    let bimodal = total > 0
        && neutral as u64 * 100 >= u64::from(total) * 95
        && u64::from(peaks[14] + peaks[15]) * 100 >= u64::from(total) * 95;
    result.mode = if unique <= 2 || bimodal {
        Mode::LineArt
    } else if dominant <= 16 {
        Mode::Logo
    } else if dominant <= 64 {
        Mode::Illustration
    } else {
        Mode::Photo
    };
    result.colors = match result.mode {
        Mode::LineArt => 2,
        Mode::Logo | Mode::Illustration => dominant.clamp(2, 64) as u16,
        Mode::Photo => 24,
    };
    if result.is_pixel_art {
        result.colors = unique.clamp(2, 64) as u16;
    }
    result
}
