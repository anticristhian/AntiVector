use av_core::{
    color::{oklab, LabelMap, Palette, PaletteColor},
    detect::Detection,
    params::DenoiseKind,
    Control, Mode, Params, RgbaImage,
};

fn detection(image: &RgbaImage) -> Detection {
    Detection {
        mode: Mode::Logo,
        colors: 3,
        has_alpha: image.has_alpha(),
        is_jpeg_like: false,
        is_pixel_art: false,
        width: image.w,
        height: image.h,
    }
}
fn params() -> Params {
    let mut p = Params {
        denoise: 0.0,
        ..Params::default()
    };
    p.experimental.quality_v5 = true;
    p.experimental.upscale_small = false;
    p
}
fn palette(colors: &[[u8; 3]]) -> Palette {
    Palette {
        colors: colors
            .iter()
            .enumerate()
            .map(|(i, &rgb)| PaletteColor {
                id: i as u16,
                rgb,
                oklab: oklab::from_srgb8(rgb),
                locked: false,
            })
            .collect(),
    }
}

#[test]
fn alpha_cleanup_and_bicubic_coordinates() {
    let image =
        RgbaImage::new(3, 1, vec![[255, 0, 0, 255], [0, 0, 0, 128], [0, 0, 255, 0]]).unwrap();
    let prepared =
        av_core::stages::preprocess(&image, &params(), &detection(&image), &Control::default())
            .unwrap();
    assert_eq!(
        prepared.image.px,
        vec![[255, 0, 0, 255], [187, 0, 0, 255], [0; 4]]
    );
    let image = RgbaImage::new(1, 1, vec![[20, 100, 230, 255]]).unwrap();
    let mut p = params();
    p.experimental.upscale_small = true;
    let prepared =
        av_core::stages::preprocess(&image, &p, &detection(&image), &Control::default()).unwrap();
    assert_eq!(prepared.scale, 2);
    assert_eq!(
        prepared.image,
        (RgbaImage::new(2, 2, vec![[20, 100, 230, 255]; 4]).unwrap())
    );
}

#[test]
fn denoise_variants_are_deterministic_and_keep_alpha_mask() {
    let image = RgbaImage::new(
        16,
        16,
        (0..256)
            .map(|i| {
                [
                    (i * 7) as u8,
                    (i * 13) as u8,
                    (i * 31) as u8,
                    if i % 17 == 0 { 0 } else { 255 },
                ]
            })
            .collect(),
    )
    .unwrap();
    for kind in [
        DenoiseKind::Bilateral,
        DenoiseKind::MeanShift,
        DenoiseKind::Guided,
    ] {
        let mut p = params();
        p.denoise = 0.5;
        p.experimental.denoise_kind = kind;
        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    av_core::stages::preprocess(&image, &p, &detection(&image), &Control::default())
                        .unwrap()
                        .image
                })
        };
        let a = run(1);
        assert_eq!(a, run(4));
        for (a, b) in a.px.iter().zip(&image.px) {
            assert_eq!(a[3], b[3]);
        }
    }
}

#[test]
fn resolves_gray_transition_without_a_halo_region() {
    let colors = palette(&[[0, 0, 0], [128, 128, 128], [255, 255, 255]]);
    let image = RgbaImage::new(
        3,
        3,
        (0..9)
            .map(|i| {
                let rgb = colors.colors[i % 3].rgb;
                [rgb[0], rgb[1], rgb[2], 255]
            })
            .collect(),
    )
    .unwrap();
    let labels = LabelMap {
        w: 3,
        h: 3,
        labels: (0..9).map(|i| (i % 3) as u16).collect(),
    };
    let result =
        av_core::stages::resolve_aa(&image, &colors, &labels, &params(), &Control::default())
            .unwrap();
    assert_eq!(result.palette.colors.len(), 2);
    let regions =
        av_core::stages::label(&result.labels, Some(&image), &Control::default()).unwrap();
    assert_eq!(regions.regions.len(), 2);
    assert_eq!(result.mixture_pixels.iter().filter(|&&p| p).count(), 3);
}

#[test]
fn auto_keeps_a_minority_brand_color_and_snaps_exact_majorities() {
    let brand = [24, 48, 96];
    let accent = [208, 104, 32];
    let mut pixels = vec![[255, 255, 255, 255]; 10_000];
    for y in 12..88 {
        for x in 12..40 {
            pixels[y * 100 + x] = [brand[0], brand[1], brand[2], 255];
        }
    }
    for y in 12..30 {
        for x in 70..88 {
            pixels[y * 100 + x] = [accent[0], accent[1], accent[2], 255];
        }
    }
    let image = RgbaImage::new(100, 100, pixels).unwrap();
    let (palette, _) = av_core::stages::quantize(&image, &params(), &Control::default()).unwrap();
    assert!(palette.colors.iter().any(|c| c.rgb == brand));
    assert!(palette.colors.iter().any(|c| c.rgb == accent));
    assert!(palette.colors.len() >= 3);
    let image = RgbaImage::new(
        20,
        1,
        (0..20)
            .map(|i| {
                if i < 8 {
                    [10, 50, 200, 255]
                } else if i < 10 {
                    [50, 80, 160, 255]
                } else {
                    [220, 0, 0, 255]
                }
            })
            .collect(),
    )
    .unwrap();
    let mut p = params();
    p.colors = av_core::ColorCount::Fixed(2);
    let (palette, _) = av_core::stages::quantize(&image, &p, &Control::default()).unwrap();
    assert!(palette.colors.iter().any(|c| c.rgb == [10, 50, 200]));
}

#[test]
fn high_contrast_one_pixel_line_survives_noise_merge_at_detail_half() {
    let colors = palette(&[[255, 255, 255], [0, 0, 0]]);
    let mut labels = vec![0; 100];
    for x in 4..16 {
        labels[2 * 20 + x] = 1;
    }
    let labels = LabelMap {
        w: 20,
        h: 5,
        labels,
    };
    let ctl = Control::default();
    let map = av_core::stages::label(&labels, None, &ctl).unwrap();
    let mut p = params();
    p.min_region_area = 30;
    p.detail = 0.5;
    assert_eq!(
        av_core::stages::merge_small(&map, &colors, &p, None, &ctl)
            .unwrap()
            .regions
            .len(),
        2
    );
    p.detail = 0.4;
    assert_eq!(
        av_core::stages::merge_small(&map, &colors, &p, None, &ctl)
            .unwrap()
            .regions
            .len(),
        1
    );
}

#[test]
fn quality_pipeline_restores_original_dimensions() {
    let pixels = (0..32 * 24)
        .map(|i| {
            let x = i % 32;
            let y = i / 32;
            if x == 0 || y == 0 || x == 31 || y == 23 {
                [255; 4]
            } else {
                [(x * 7) as u8, (y * 9) as u8, ((x + y) * 4) as u8, 255]
            }
        })
        .collect();
    let image = RgbaImage::new(32, 24, pixels).unwrap();
    let mut p = params();
    p.experimental.upscale_small = true;
    p.colors = av_core::ColorCount::Fixed(4);
    let ctl = Control::default();
    let stages = av_core::pipeline::polygon_stages(&image, &p, &ctl).unwrap();
    assert_eq!(stages.scale, 2);
    assert_eq!((stages.preprocessed.w, stages.preprocessed.h), (64, 48));
    let doc = av_core::pipeline::finish(&stages, &ctl).unwrap();
    assert_eq!((doc.width, doc.height), (32.0, 24.0));
}

#[test]
fn aa_logo_has_only_ground_truth_regions_after_full_segmentation() {
    let mut pixels = vec![[255; 4]; 24 * 24];
    for y in 5..19 {
        for x in 5..19 {
            pixels[y * 24 + x] = if x == 5 || y == 5 || x == 18 || y == 18 {
                [128, 128, 128, 255]
            } else {
                [0, 0, 0, 255]
            };
        }
    }
    let image = RgbaImage::new(24, 24, pixels).unwrap();
    let mut p = params();
    p.colors = av_core::ColorCount::Fixed(3);
    p.min_region_area = 2;
    let stages = av_core::pipeline::polygon_stages(&image, &p, &Control::default()).unwrap();
    assert_eq!(stages.merged.regions.len(), 2);
    assert_eq!(stages.palette.colors.len(), 2);
    assert_eq!(stages.quant_palette.colors.len(), 3);
}

#[test]
fn multicolor_frame_keeps_original_sampling_and_pixel_art_rare_accents() {
    let image = RgbaImage::new(
        3,
        3,
        vec![
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            [255; 4],
            [255; 4],
            [20, 80, 120, 255],
            [255; 4],
            [255; 4],
            [255; 4],
            [255; 4],
        ],
    )
    .unwrap();
    let mut p = params();
    p.experimental.upscale_small = true;
    assert_eq!(
        av_core::preprocess::preprocess(&image, &p, &detection(&image), &Control::default())
            .unwrap()
            .scale,
        1
    );
    let mut pixels = vec![[255; 4]; 100 * 100];
    pixels[5050] = [255, 80, 0, 255];
    for pixel in &mut pixels[3000..6000] {
        if *pixel == [255; 4] {
            *pixel = [0, 0, 0, 255];
        }
    }
    let image = RgbaImage::new(100, 100, pixels).unwrap();
    let detected = av_core::detect(&image);
    assert!(detected.is_pixel_art);
    assert_eq!(detected.colors, 3);
}
