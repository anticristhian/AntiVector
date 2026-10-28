//! Unpremultiplied sRGB RGBA8 input and allocation guards.

use crate::CoreError;

/// Default budget, in pixels (64 megapixels in decimal units).
pub const DEFAULT_PIXEL_LIMIT: u64 = 64_000_000;

/// Validate dimensions before decoding or allocating a pixel buffer.
pub fn validate_dimensions(w: u32, h: u32, limit: u64) -> Result<usize, CoreError> {
    if w == 0 || h == 0 {
        return Err(CoreError::EmptyImage);
    }
    let count = u64::from(w) * u64::from(h);
    if count > limit || count > (isize::MAX as u64) / 4 {
        return Err(CoreError::ImageTooLarge { w, h, limit });
    }
    usize::try_from(count).map_err(|_| CoreError::ImageTooLarge { w, h, limit })
}

/// Row-major, unpremultiplied sRGB pixels. Public fields are revalidated at stage entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbaImage {
    pub w: u32,
    pub h: u32,
    pub px: Vec<[u8; 4]>,
}

impl RgbaImage {
    /// Construct with the default 64 MP budget and exact buffer-length validation.
    pub fn new(w: u32, h: u32, px: Vec<[u8; 4]>) -> Result<Self, CoreError> {
        let image = Self { w, h, px };
        image.validate(DEFAULT_PIXEL_LIMIT)?;
        Ok(image)
    }

    /// Validate dimensions and buffer length, optionally overriding the pixel budget.
    pub fn validate(&self, limit: u64) -> Result<(), CoreError> {
        let expected = validate_dimensions(self.w, self.h, limit)?;
        if self.px.len() != expected {
            return Err(CoreError::InvalidParams(format!(
                "pixel buffer has {} entries; expected {expected}",
                self.px.len()
            )));
        }
        Ok(())
    }

    /// Match the detection contract: any alpha value below 250 indicates alpha.
    pub fn has_alpha(&self) -> bool {
        self.px.iter().any(|pixel| pixel[3] < 250)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_malformed_inputs() {
        for (w, h) in [(0, 0), (0, 1), (1, 0)] {
            assert_eq!(RgbaImage::new(w, h, vec![]), Err(CoreError::EmptyImage));
        }
        assert!(matches!(
            RgbaImage::new(2, 2, vec![[0; 4]]),
            Err(CoreError::InvalidParams(_))
        ));
        assert!(matches!(
            validate_dimensions(u32::MAX, u32::MAX, DEFAULT_PIXEL_LIMIT),
            Err(CoreError::ImageTooLarge { .. })
        ));
    }

    #[test]
    fn limits_without_large_allocation() {
        assert_eq!(
            validate_dimensions(8000, 8000, DEFAULT_PIXEL_LIMIT),
            Ok(64_000_000)
        );
        assert!(validate_dimensions(8001, 8000, DEFAULT_PIXEL_LIMIT).is_err());
        assert_eq!(validate_dimensions(8001, 8000, 65_000_000), Ok(64_008_000));
    }

    #[test]
    fn transparent_and_uniform_are_valid() {
        let mut image = RgbaImage::new(1, 1, vec![[0, 0, 0, 0]]).unwrap();
        assert!(image.has_alpha());
        image.px[0][3] = 250;
        assert!(!image.has_alpha());
        image.px.clear();
        assert!(image.validate(DEFAULT_PIXEL_LIMIT).is_err());
    }
}
