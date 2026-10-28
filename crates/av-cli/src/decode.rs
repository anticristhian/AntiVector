//! File I/O stays in the adapter, outside av-core.

use av_core::{raster::validate_dimensions, CoreError, RgbaImage};
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use std::{
    fmt,
    fs::File,
    io::{BufRead, BufReader, Seek},
    path::Path,
};

#[derive(Debug)]
pub enum DecodeError {
    Io(std::io::Error),
    Decode(image::ImageError),
    Input(CoreError),
}

impl DecodeError {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Io(_) => 4,
            Self::Decode(_) => 3,
            Self::Input(_) => 2,
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O: {e}"),
            Self::Decode(e) => write!(f, "decode: {e}"),
            Self::Input(e) => e.fmt(f),
        }
    }
}

pub fn load(path: &Path, limit: u64) -> Result<RgbaImage, DecodeError> {
    let file = File::open(path).map_err(DecodeError::Io)?;
    decode(BufReader::new(file), limit)
}

fn decode(reader: impl BufRead + Seek, limit: u64) -> Result<RgbaImage, DecodeError> {
    // Guess by content, allowing supported images without a conventional extension.
    let mut reader = ImageReader::new(reader)
        .with_guessed_format()
        .map_err(DecodeError::Io)?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(limit.saturating_mul(16).saturating_add(200_000_000));
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(DecodeError::Decode)?;
    let (w, h) = decoder.dimensions();
    // Validate pixel count BEFORE the decoder allocates its full output buffer.
    validate_dimensions(w, h, limit).map_err(DecodeError::Input)?;
    // image's GIF ImageDecoder decodes the first composited frame only.
    // DynamicImage performs the 16-bit -> RGBA8 conversion for PNG/TIFF.
    let rgba = DynamicImage::from_decoder(decoder)
        .map_err(DecodeError::Decode)?
        .into_rgba8();
    let result = RgbaImage {
        w,
        h,
        px: rgba.pixels().map(|pixel| pixel.0).collect(),
    };
    result.validate(limit).map_err(DecodeError::Input)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{codecs::gif::GifEncoder, Frame, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;

    fn bytes(image: &DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        image.write_to(&mut buffer, format).unwrap();
        buffer.into_inner()
    }

    #[test]
    fn supported_formats_and_corruption() {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(3, 2, image::Rgb([255, 0, 0])));
        for format in [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Bmp,
            ImageFormat::Gif,
            ImageFormat::Tiff,
            ImageFormat::WebP,
        ] {
            let output = decode(Cursor::new(bytes(&image, format)), 100).unwrap();
            assert_eq!((output.w, output.h), (3, 2), "{format:?}");
            assert!(!output.has_alpha(), "{format:?}");
        }
        assert!(decode(Cursor::new(b"corrupted"), 100).is_err());
        assert!(decode(Cursor::new(bytes(&image, ImageFormat::Png)), 5).is_err());
    }

    #[test]
    fn sixteen_bit_png_and_tiff_to_eight_bit() {
        let image = DynamicImage::ImageRgba16(ImageBuffer::from_pixel(
            1,
            1,
            Rgba([65535, 32896, 0, 16448]),
        ));
        for format in [ImageFormat::Png, ImageFormat::Tiff] {
            let output = decode(Cursor::new(bytes(&image, format)), 100).unwrap();
            assert_eq!(output.px, vec![[255, 128, 0, 64]]);
            assert!(output.has_alpha());
        }
    }

    #[test]
    fn gif_uses_first_frame() {
        let mut buffer = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut buffer);
            for rgb in [[255, 0, 0, 255], [0, 0, 255, 255]] {
                encoder
                    .encode_frame(Frame::new(ImageBuffer::from_pixel(2, 2, Rgba(rgb))))
                    .unwrap();
            }
        }
        let output = decode(Cursor::new(buffer), 100).unwrap();
        assert_eq!(output.px, vec![[255, 0, 0, 255]; 4]);
    }

    #[test]
    fn alpha_png_roundtrip() {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(1, 1, Rgba([10, 20, 30, 0])));
        let output = decode(Cursor::new(bytes(&image, ImageFormat::Png)), 1).unwrap();
        assert_eq!(output.px, vec![[10, 20, 30, 0]]);
    }
}
