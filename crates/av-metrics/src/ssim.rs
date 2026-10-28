//! SSIM on normalized sRGB luminance: Gaussian 11x11, sigma=1.5, replicated borders.

use av_core::{CoreError, RgbaImage};

fn gaussian(values: &[f64], w: usize, h: usize) -> Vec<f64> {
    let mut weights: [f64; 11] =
        std::array::from_fn(|i| (-((i as f64 - 5.0).powi(2)) / (2.0 * 1.5_f64.powi(2))).exp());
    let sum: f64 = weights.iter().sum();
    for weight in &mut weights {
        *weight /= sum;
    }
    let mut horizontal = vec![0.0; values.len()];
    let mut output = vec![0.0; values.len()];
    for y in 0..h {
        for x in 0..w {
            for (j, weight) in weights.iter().enumerate() {
                let sx = (x as isize + j as isize - 5).clamp(0, w as isize - 1) as usize;
                horizontal[y * w + x] += weight * values[y * w + sx];
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            for (j, weight) in weights.iter().enumerate() {
                let sy = (y as isize + j as isize - 5).clamp(0, h as isize - 1) as usize;
                output[y * w + x] += weight * horizontal[sy * w + x];
            }
        }
    }
    output
}

/// Structural similarity in [-1,1], with valid behavior for images smaller than 11px.
pub fn ssim(a: &RgbaImage, b: &RgbaImage) -> Result<f64, CoreError> {
    a.validate(av_core::raster::DEFAULT_PIXEL_LIMIT)?;
    b.validate(av_core::raster::DEFAULT_PIXEL_LIMIT)?;
    if (a.w, a.h) != (b.w, b.h) {
        return Err(CoreError::InvalidParams("SSIM dimensions differ".into()));
    }
    let luminance = |image: &RgbaImage| {
        image
            .px
            .iter()
            .map(|&p| {
                let [r, g, b] = crate::composite_white(p).map(f64::from);
                (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
            })
            .collect::<Vec<_>>()
    };
    let x = luminance(a);
    let y = luminance(b);
    let w = a.w as usize;
    let h = a.h as usize;
    let mx = gaussian(&x, w, h);
    let my = gaussian(&y, w, h);
    let xx = gaussian(&x.iter().map(|v| v * v).collect::<Vec<_>>(), w, h);
    let yy = gaussian(&y.iter().map(|v| v * v).collect::<Vec<_>>(), w, h);
    let xy = gaussian(
        &x.iter().zip(&y).map(|(a, b)| a * b).collect::<Vec<_>>(),
        w,
        h,
    );
    let mut total = 0.0;
    for i in 0..x.len() {
        let vx = (xx[i] - mx[i] * mx[i]).max(0.0);
        let vy = (yy[i] - my[i] * my[i]).max(0.0);
        let cov = xy[i] - mx[i] * my[i];
        total += ((2.0 * mx[i] * my[i] + 0.01_f64.powi(2)) * (2.0 * cov + 0.03_f64.powi(2)))
            / ((mx[i].powi(2) + my[i].powi(2) + 0.01_f64.powi(2)) * (vx + vy + 0.03_f64.powi(2)));
    }
    Ok((total / x.len() as f64).clamp(-1.0, 1.0))
}
