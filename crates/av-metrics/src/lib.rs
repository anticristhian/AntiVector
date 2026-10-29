//! Fidelity, coverage and geometry metrics for rasterized vector documents.

pub mod delta_e;
pub mod geometry;
pub mod render;
pub mod ssim;

use av_core::{CoreError, RgbaImage};

/// Fidelity over an opaque mask, structural similarity over white-composited pixels.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Fidelity {
    pub delta_e_mean: f64,
    pub delta_e_p95: f64,
    pub ssim: f64,
    pub gap_pixels: usize,
}

/// Compare AA and non-AA renders; alpha coverage avoids magenta-color collisions.
pub fn compare(
    original: &RgbaImage,
    render: &RgbaImage,
    coverage: &RgbaImage,
    alpha_cut: u8,
) -> Result<Fidelity, CoreError> {
    for image in [original, render, coverage] {
        image.validate(av_core::raster::DEFAULT_PIXEL_LIMIT)?;
    }
    if (original.w, original.h) != (render.w, render.h)
        || (original.w, original.h) != (coverage.w, coverage.h)
    {
        return Err(CoreError::InvalidParams("metric dimensions differ".into()));
    }
    // Exact equality proves both fidelity metrics; still scan actual full-resolution
    // coverage. Avoid allocating Gaussian work buffers for a lossless 64MP render.
    if original == render {
        return Ok(Fidelity {
            delta_e_mean: 0.0,
            delta_e_p95: 0.0,
            ssim: 1.0,
            gap_pixels: original
                .px
                .iter()
                .zip(&coverage.px)
                .filter(|(a, c)| a[3] >= alpha_cut && c[3] == 0)
                .count(),
        });
    }
    let mut errors = Vec::new();
    let mut gaps = 0;
    for ((a, b), c) in original.px.iter().zip(&render.px).zip(&coverage.px) {
        if a[3] >= alpha_cut {
            errors.push(delta_e::ciede2000(
                delta_e::srgb_to_lab(composite_white(*a)),
                delta_e::srgb_to_lab(composite_white(*b)),
            ));
            if c[3] == 0 {
                gaps += 1;
            }
        }
    }
    let mean = if errors.is_empty() {
        0.0
    } else {
        errors.iter().sum::<f64>() / errors.len() as f64
    };
    errors.sort_by(f64::total_cmp);
    let p95 = if errors.is_empty() {
        0.0
    } else {
        errors[((errors.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)]
    };
    Ok(Fidelity {
        delta_e_mean: mean,
        delta_e_p95: p95,
        ssim: ssim::ssim(original, render)?,
        gap_pixels: gaps,
    })
}

/// sRGB composition over white for reproducible fidelity and luminance comparison.
pub fn composite_white(pixel: [u8; 4]) -> [u8; 3] {
    let alpha = u32::from(pixel[3]);
    std::array::from_fn(|ch| {
        ((u32::from(pixel[ch]) * alpha + 255 * (255 - alpha) + 127) / 255) as u8
    })
}
