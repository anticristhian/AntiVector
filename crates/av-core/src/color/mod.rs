//! Perceptual color representation and deterministic palette operations.

pub mod antialias;
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

/// Compact labels in raster order, retaining unused forced colors after used ones.
pub fn compact_palette(
    palette: &Palette,
    labels: &mut LabelMap,
) -> Result<Palette, crate::CoreError> {
    labels.validate()?;
    let mut remap = vec![TRANSPARENT; palette.colors.len()];
    let mut colors = Vec::new();
    for old in labels.labels.iter_mut().filter(|id| **id != TRANSPARENT) {
        let index = *old as usize;
        let color = palette
            .colors
            .get(index)
            .ok_or_else(|| crate::CoreError::InvalidParams("label palette reference".into()))?;
        if remap[index] == TRANSPARENT {
            let mut color = color.clone();
            color.id = colors.len() as u16;
            remap[index] = color.id;
            colors.push(color);
        }
        *old = remap[index];
    }
    for (i, color) in palette.colors.iter().enumerate() {
        if color.locked && remap[i] == TRANSPARENT {
            let mut color = color.clone();
            color.id = colors.len() as u16;
            colors.push(color);
        }
    }
    Ok(Palette { colors })
}

/// Remove palette entries eliminated by region merging, preserving forced colors.
pub fn compact_regions(
    palette: &Palette,
    map: &mut crate::segment::RegionMap,
) -> Result<Palette, crate::CoreError> {
    map.validate()?;
    let mut remap = vec![TRANSPARENT; palette.colors.len()];
    let mut colors = Vec::new();
    for region in &mut map.regions {
        if region.color == TRANSPARENT {
            continue;
        }
        let index = region.color as usize;
        let original = palette
            .colors
            .get(index)
            .ok_or_else(|| crate::CoreError::InvalidParams("region palette reference".into()))?;
        if remap[index] == TRANSPARENT {
            let mut color = original.clone();
            color.id = colors.len() as u16;
            remap[index] = color.id;
            colors.push(color);
        }
        region.color = remap[index];
    }
    for (i, color) in palette.colors.iter().enumerate() {
        if color.locked && remap[i] == TRANSPARENT {
            let mut color = color.clone();
            color.id = colors.len() as u16;
            colors.push(color);
        }
    }
    Ok(Palette { colors })
}
