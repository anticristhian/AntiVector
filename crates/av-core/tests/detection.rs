use av_core::{detect, Mode, RgbaImage};

#[test]
fn detector_retains_minority_accents_and_handles_empty_input() {
    let mut pixels = vec![[255, 255, 255, 255]; 10_000];
    for pixel in &mut pixels[5000..9700] {
        *pixel = [24, 48, 96, 255];
    }
    for pixel in &mut pixels[9700..] {
        *pixel = [208, 104, 32, 255];
    }
    let detection = detect(&RgbaImage::new(100, 100, pixels).unwrap());
    assert_eq!(detection.colors, 3);
    assert_eq!(detection.mode, Mode::Logo);
    assert!(!detection.is_jpeg_like);
    let result = detect(&RgbaImage {
        w: 0,
        h: 0,
        px: vec![],
    });
    assert_eq!(result.colors, 2);
}
