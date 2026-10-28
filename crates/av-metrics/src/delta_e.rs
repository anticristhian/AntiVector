//! CIELAB D65 and CIEDE2000 (kL=kC=kH=1), independently implemented from equations.
//! Reference: Sharma, Wu & Dalal, Color Research & Application 30(1), 2005.

/// Convert sRGB8 to CIELAB with D65 reference white.
pub fn srgb_to_lab(rgb: [u8; 3]) -> [f64; 3] {
    let [r, g, b] = rgb.map(|c| av_core::color::oklab::linear_lut()[c as usize]);
    let xyz = [
        (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047,
        0.2126729 * r + 0.7151522 * g + 0.0721750 * b,
        (0.0193339 * r + 0.1191920 * g + 0.9503041 * b) / 1.08883,
    ];
    let [x, y, z] = xyz.map(|v| {
        if v > 216.0 / 24389.0 {
            v.cbrt()
        } else {
            (24389.0 / 27.0 * v + 16.0) / 116.0
        }
    });
    [116.0 * y - 16.0, 500.0 * (x - y), 200.0 * (y - z)]
}

/// Perceptual CIEDE2000 distance between two [L*,a*,b*] values.
pub fn ciede2000([l1, a1, b1]: [f64; 3], [l2, a2, b2]: [f64; 3]) -> f64 {
    let cbar = (a1.hypot(b1) + a2.hypot(b2)) / 2.0;
    let g = 0.5 * (1.0 - (cbar.powi(7) / (cbar.powi(7) + 25_f64.powi(7))).sqrt());
    let a1p = (1.0 + g) * a1;
    let a2p = (1.0 + g) * a2;
    let c1 = a1p.hypot(b1);
    let c2 = a2p.hypot(b2);
    let hue = |a: f64, b: f64| {
        if a == 0.0 && b == 0.0 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        }
    };
    let h1 = hue(a1p, b1);
    let h2 = hue(a2p, b2);
    let diff = h2 - h1;
    let dh = if c1 * c2 == 0.0 {
        0.0
    } else if diff > 180.0 {
        diff - 360.0
    } else if diff < -180.0 {
        diff + 360.0
    } else {
        diff
    };
    let dl = l2 - l1;
    let dc = c2 - c1;
    let d_h = 2.0 * (c1 * c2).sqrt() * (dh / 2.0).to_radians().sin();
    let lm = (l1 + l2) / 2.0;
    let cm = (c1 + c2) / 2.0;
    let hm = if c1 * c2 == 0.0 {
        h1 + h2
    } else if diff.abs() <= 180.0 {
        (h1 + h2) / 2.0
    } else if h1 + h2 < 360.0 {
        (h1 + h2 + 360.0) / 2.0
    } else {
        (h1 + h2 - 360.0) / 2.0
    };
    let cos = |deg: f64| deg.to_radians().cos();
    let t = 1.0 - 0.17 * cos(hm - 30.0) + 0.24 * cos(2.0 * hm) + 0.32 * cos(3.0 * hm + 6.0)
        - 0.20 * cos(4.0 * hm - 63.0);
    let sl = 1.0 + 0.015 * (lm - 50.0).powi(2) / (20.0 + (lm - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cm;
    let sh = 1.0 + 0.015 * cm * t;
    let theta = 30.0 * (-((hm - 275.0) / 25.0).powi(2)).exp();
    let rt = -2.0
        * (cm.powi(7) / (cm.powi(7) + 25_f64.powi(7))).sqrt()
        * (2.0 * theta).to_radians().sin();
    let a = dl / sl;
    let b = dc / sc;
    let c = d_h / sh;
    (a * a + b * b + c * c + rt * b * c).max(0.0).sqrt()
}
