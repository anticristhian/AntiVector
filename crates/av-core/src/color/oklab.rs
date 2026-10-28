//! OKLab transforms using Ottosson's 2021 matrices:
//! <https://bottosson.github.io/posts/oklab/> (public-domain mathematical formulas).
//! All computations use f64; transfer functions operate on normalized sRGB.

use std::sync::OnceLock;

/// Perceptual D65 lightness and opponent color components.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Oklab {
    pub l: f64,
    pub a: f64,
    pub b: f64,
}

/// Decode a normalized sRGB channel into linear light.
pub fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Encode a linear-light channel into normalized sRGB; does not clip the gamut.
pub fn linear_to_srgb(v: f64) -> f64 {
    if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Shared 256-entry sRGB8 linearization table, initialized once per process.
pub fn linear_lut() -> &'static [f64; 256] {
    static LUT: OnceLock<[f64; 256]> = OnceLock::new();
    LUT.get_or_init(|| std::array::from_fn(|i| srgb_to_linear(i as f64 / 255.0)))
}

/// Convert linear-light sRGB into OKLab.
pub fn linear_to_oklab([r, g, b]: [f64; 3]) -> Oklab {
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    Oklab {
        l: 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        a: 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        b: 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    }
}

/// Convert OKLab into unclipped linear-light sRGB.
pub fn oklab_to_linear(c: Oklab) -> [f64; 3] {
    let l = (c.l + 0.3963377774 * c.a + 0.2158037573 * c.b).powi(3);
    let m = (c.l - 0.1055613458 * c.a - 0.0638541728 * c.b).powi(3);
    let s = (c.l - 0.0894841775 * c.a - 1.2914855480 * c.b).powi(3);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
}

/// Convert an sRGB8 triplet using the linearization lookup table.
pub fn from_srgb8(rgb: [u8; 3]) -> Oklab {
    let lut = linear_lut();
    linear_to_oklab(rgb.map(|v| lut[usize::from(v)]))
}

/// Convert to normalized sRGB without clipping, useful for error measurement.
pub fn to_srgb(c: Oklab) -> [f64; 3] {
    oklab_to_linear(c).map(linear_to_srgb)
}

/// Convert to sRGB8, clipping out-of-gamut channels and rounding to nearest byte.
pub fn to_srgb8(c: Oklab) -> [u8; 3] {
    to_srgb(c).map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

impl Oklab {
    /// Squared Euclidean distance for palette assignment.
    pub fn distance_squared(self, other: Self) -> f64 {
        (self.l - other.l).powi(2) + (self.a - other.a).powi(2) + (self.b - other.b).powi(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_colors() {
        for (rgb, expected) in [
            ([0, 0, 0], [0.0, 0.0, 0.0]),
            ([255, 255, 255], [1.0, 0.0, 0.0]),
            ([255, 0, 0], [0.62795536, 0.22486306, 0.12584630]),
            ([0, 255, 0], [0.86643961, -0.23388757, 0.17949848]),
            ([0, 0, 255], [0.45201372, -0.03245698, -0.31152815]),
        ] {
            let c = from_srgb8(rgb);
            for (v, r) in [c.l, c.a, c.b].into_iter().zip(expected) {
                assert!((v - r).abs() < 1e-6, "{rgb:?}: {c:?}");
            }
        }
    }

    #[test]
    fn roundtrip_100k_deterministic_colors() {
        let mut state = 0x12345678_u32;
        let mut worst: f64 = 0.0;
        for _ in 0..100_000 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let rgb = [(state >> 16) as u8, (state >> 8) as u8, state as u8];
            let lab = from_srgb8(rgb);
            for (actual, byte) in to_srgb(lab).into_iter().zip(rgb) {
                worst = worst.max((actual - f64::from(byte) / 255.0).abs());
            }
            assert_eq!(to_srgb8(lab), rgb);
        }
        println!("100000 sRGB colors: maximum normalized channel roundtrip error = {worst:.12}");
        assert!(worst < 1e-4);
    }

    #[test]
    fn transfer_function_and_lut() {
        for i in 0..256 {
            let v = i as f64 / 255.0;
            assert!((linear_to_srgb(linear_lut()[i]) - v).abs() < 1e-12);
        }
    }
}
