//! Perceptual color representation and deterministic palette operations.

pub mod oklab;
pub mod quantize;

use oklab::Oklab;

/// Sentinel color shared by all transparent pixels.
pub const TRANSPARENT: u16 = u16::MAX;

/// Stable palette entry; id is its dense index in Palette::colors.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PaletteColor {
    pub id: u16,
    pub rgb: [u8; 3],
    pub oklab: Oklab,
    pub locked: bool,
}

/// Ordered perceptual palette with stable dense ids.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Palette {
    pub colors: Vec<PaletteColor>,
}

/// Row-major palette indices; TRANSPARENT denotes excluded pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelMap {
    pub w: u32,
    pub h: u32,
    pub labels: Vec<u16>,
}

impl LabelMap {
    /// Validate user-visible dimensions and label buffer before topology stages.
    pub fn validate(&self) -> Result<(), crate::CoreError> {
        let size =
            crate::raster::validate_dimensions(self.w, self.h, crate::raster::DEFAULT_PIXEL_LIMIT)?;
        if size != self.labels.len() {
            return Err(crate::CoreError::InvalidParams(
                "label buffer length".into(),
            ));
        }
        Ok(())
    }
}
