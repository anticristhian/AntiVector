//! Versioned, validated parameters and documented preset defaults.

use crate::{detect::Detection, CoreError};

/// Vectorization mode; pixel-art is a preset rather than a separate engine mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Logo,
    Illustration,
    Photo,
    LineArt,
}

/// Automatic palette selection or an explicit 2..=256 color budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorCount {
    Auto,
    Fixed(u16),
}

/// Harness-only alternatives until the ADR-007 comparison has been recorded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quantizer {
    #[default]
    Kmeans,
    MedianCut,
    Wu,
}

/// Denoising alternatives compared by the F5 harness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenoiseKind {
    #[default]
    Bilateral,
    MeanShift,
    Guided,
}

/// Pixel-art baseline versus scale-aware corner fitting (F5 experiment).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelArtStrategy {
    #[default]
    PreserveGrid,
    ScaleCorners,
}

/// Explicit opt-ins for experiments; defaults preserve the documented algorithm.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentalParams {
    /// Opt-in exporter composition experiment; leaves all engine stages unchanged.
    #[serde(default)]
    pub opaque_canvas_overlap: bool,
    #[serde(default)]
    pub quality_v5: bool,
    #[serde(default)]
    pub denoise_kind: DenoiseKind,
    #[serde(default)]
    pub force_pixel_art: bool,
    #[serde(default)]
    pub pixel_art_strategy: PixelArtStrategy,
    #[serde(default = "default_upscale")]
    pub upscale_small: bool,
    #[serde(default = "default_corner_radius")]
    pub corner_recovery_radius: f64,
    #[serde(default)]
    pub curve_fit: bool,
    #[serde(default)]
    pub quantizer: Quantizer,
}

fn default_upscale() -> bool {
    true
}

fn default_corner_radius() -> f64 {
    1.5
}
impl Default for ExperimentalParams {
    fn default() -> Self {
        Self {
            opaque_canvas_overlap: false,
            quality_v5: true,
            denoise_kind: DenoiseKind::Bilateral,
            force_pixel_art: false,
            pixel_art_strategy: PixelArtStrategy::PreserveGrid,
            upscale_small: true,
            curve_fit: true,
            quantizer: Quantizer::Kmeans,
            corner_recovery_radius: 1.5,
        }
    }
}

/// Complete serializable v1 engine parameters. Validate before using public fields.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    #[serde(default)]
    pub experimental: ExperimentalParams,
    pub version: u32,
    pub mode: Mode,
    pub colors: ColorCount,
    pub palette_lock: Vec<[u8; 3]>,
    pub denoise: f32,
    pub min_region_area: u32,
    pub antialias_resolve: bool,
    pub corner_threshold_deg: f32,
    pub smoothness: f32,
    pub fit_tolerance_px: f32,
    pub detail: f32,
    pub transparency_alpha_cut: u8,
    pub seed: u64,
}

impl Params {
    /// Load a bundled preset with validation. No filesystem I/O is performed.
    pub fn preset(name: &str) -> Result<Self, CoreError> {
        let json = match name {
            "logo" => include_str!("../presets/logo.json"),
            "illustration" => include_str!("../presets/illustration.json"),
            "photo" => include_str!("../presets/photo.json"),
            "lineart" => include_str!("../presets/lineart.json"),
            "pixelart" => include_str!("../presets/pixelart.json"),
            _ => return Err(CoreError::InvalidParams(format!("unknown preset: {name}"))),
        };
        Self::from_json(json)
    }

    /// Deserialize and reject unsupported versions, unknown keys and invalid values.
    pub fn from_json(json: &str) -> Result<Self, CoreError> {
        let params: Self =
            serde_json::from_str(json).map_err(|e| CoreError::InvalidParams(e.to_string()))?;
        params.validate()?;
        Ok(params)
    }

    /// Validate every numeric domain, including NaN/infinity and locked-color budget.
    pub fn validate(&self) -> Result<(), CoreError> {
        let invalid = |message: &str| Err(CoreError::InvalidParams(message.into()));
        if !self.experimental.corner_recovery_radius.is_finite()
            || !(0.5..=2.0).contains(&self.experimental.corner_recovery_radius)
        {
            return invalid("experimental.corner_recovery_radius must be finite and in 0.5..=2");
        }
        if self.version != 1 {
            return invalid("unsupported parameter version");
        }
        let budget = match self.colors {
            ColorCount::Auto => {
                if self.mode == Mode::Photo {
                    128
                } else {
                    64
                }
            }
            ColorCount::Fixed(n) if (2..=256).contains(&n) => n,
            _ => return invalid("colors must be auto or 2..=256"),
        };
        if self.palette_lock.len() > usize::from(budget) {
            return invalid("locked colors exceed palette budget");
        }
        for (i, color) in self.palette_lock.iter().enumerate() {
            if self.palette_lock[..i].contains(color) {
                return invalid("duplicate locked color");
            }
        }
        for (name, value, low, high) in [
            ("denoise", self.denoise, 0.0, 1.0),
            ("smoothness", self.smoothness, 0.0, 1.0),
            ("detail", self.detail, 0.0, 1.0),
            (
                "corner_threshold_deg",
                self.corner_threshold_deg,
                30.0,
                170.0,
            ),
            ("fit_tolerance_px", self.fit_tolerance_px, 0.1, 3.0),
        ] {
            if !value.is_finite() || !(low..=high).contains(&value) {
                return Err(CoreError::InvalidParams(format!(
                    "{name} must be finite and in {low}..={high}"
                )));
            }
        }
        if self.transparency_alpha_cut == 0 {
            return invalid("transparency_alpha_cut must be 1..=255");
        }
        Ok(())
    }
}

impl Default for Params {
    fn default() -> Self {
        Self {
            experimental: ExperimentalParams::default(),
            version: 1,
            mode: Mode::Logo,
            colors: ColorCount::Auto,
            palette_lock: vec![],
            denoise: 0.2,
            min_region_area: 33,
            antialias_resolve: true,
            corner_threshold_deg: 100.0,
            smoothness: 0.5,
            fit_tolerance_px: 0.9,
            detail: 0.5,
            transparency_alpha_cut: 128,
            seed: 0,
        }
    }
}

/// Resolve initial values from a mode and measured image flags.
/// Minimum region area scales with image area; user edits are applied afterwards.
/// Detection heuristics themselves remain subject to F3 harness calibration.
pub fn resolve_defaults(mode: Mode, detection: &Detection) -> Params {
    let denoise = if detection.is_pixel_art {
        0.0
    } else {
        match mode {
            Mode::Logo if detection.is_jpeg_like => 0.5,
            Mode::Logo | Mode::LineArt => 0.2,
            Mode::Illustration => 0.3,
            Mode::Photo => 0.6,
        }
    };
    Params {
        mode,
        denoise,
        min_region_area: default_min_region_area(0.5, detection.width, detection.height),
        fit_tolerance_px: if mode == Mode::Photo { 1.35 } else { 0.9 },
        ..Params::default()
    }
}

/// Derive the documented region-area default for a validated detail value.
pub fn default_min_region_area(detail: f32, width: u32, height: u32) -> u32 {
    let area = f64::from(width) * f64::from(height);
    ((64.0 - 62.0 * f64::from(detail)) * (area / 1e6).sqrt())
        .round()
        .max(2.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_roundtrip() {
        for name in ["logo", "illustration", "photo", "lineart", "pixelart"] {
            let params = Params::preset(name).unwrap();
            assert_eq!(
                Params::from_json(&serde_json::to_string(&params).unwrap()).unwrap(),
                params
            );
        }
        assert!(Params::preset("unknown").is_err());
        assert_eq!(Params::preset("logo").unwrap(), Params::default());
    }

    #[test]
    fn rejects_invalid_numeric_domains_and_versions() {
        for value in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(Params {
                smoothness: value,
                ..Params::default()
            }
            .validate()
            .is_err());
        }
        for colors in [
            ColorCount::Fixed(0),
            ColorCount::Fixed(1),
            ColorCount::Fixed(257),
        ] {
            assert!(Params {
                colors,
                ..Params::default()
            }
            .validate()
            .is_err());
        }
        assert!(Params {
            version: 2,
            ..Params::default()
        }
        .validate()
        .is_err());
        assert!(Params {
            transparency_alpha_cut: 0,
            ..Params::default()
        }
        .validate()
        .is_err());
        assert!(Params {
            palette_lock: vec![[0; 3]; 2],
            ..Params::default()
        }
        .validate()
        .is_err());
        assert!(Params {
            colors: ColorCount::Fixed(2),
            palette_lock: vec![[0; 3], [1; 3], [2; 3]],
            ..Params::default()
        }
        .validate()
        .is_err());
        let mut json = serde_json::to_value(Params::default()).unwrap();
        json["unknown"] = true.into();
        assert!(Params::from_json(&json.to_string()).is_err());
    }

    #[test]
    fn defaults_follow_mode_flags_and_area() {
        let mut detection = Detection {
            mode: Mode::Logo,
            colors: 8,
            has_alpha: false,
            is_jpeg_like: true,
            is_pixel_art: false,
            width: 1000,
            height: 1000,
        };
        let defaults = resolve_defaults(Mode::Logo, &detection);
        assert_eq!(defaults.denoise, 0.5);
        assert_eq!(defaults.min_region_area, 33);
        assert_eq!(
            resolve_defaults(Mode::Photo, &detection).fit_tolerance_px,
            1.35
        );
        detection.is_pixel_art = true;
        detection.width = 1;
        assert_eq!(
            resolve_defaults(Mode::Illustration, &detection).denoise,
            0.0
        );
        assert_eq!(
            resolve_defaults(Mode::Illustration, &detection).min_region_area,
            2
        );
    }
}
