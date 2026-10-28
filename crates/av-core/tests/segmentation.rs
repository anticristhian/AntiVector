use av_core::{
    color::{oklab, quantize::quantize, LabelMap, Palette, PaletteColor, TRANSPARENT},
    segment::{label::label, merge::merge_small},
    ColorCount, Control, Params, RgbaImage,
};

#[test]
fn quantization_determinism_locks_and_transparency() {
    let image = RgbaImage::new(
        4,
        2,
        vec![
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            [0, 0, 0, 0],
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            [0, 0, 255, 255],
        ],
    )
    .unwrap();
    let params = Params {
        colors: ColorCount::Fixed(2),
        palette_lock: vec![[255, 0, 0]],
        ..Params::default()
    };
    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| quantize(&image, &params, &Control::default()).unwrap())
    };
    let a = run(1);
    let b = run(4);
    assert_eq!(a, b);
    assert_eq!(a.1.labels[2], TRANSPARENT);
    assert!(a.0.colors.iter().any(|c| c.rgb == [255, 0, 0] && c.locked));
    assert_eq!(a.0.colors.len(), 2);
    let transparent = RgbaImage::new(1, 1, vec![[0; 4]]).unwrap();
    assert!(
        quantize(&transparent, &Params::default(), &Control::default())
            .unwrap()
            .0
            .colors
            .is_empty()
    );
    let uniform = RgbaImage::new(1, 1, vec![[255; 4]]).unwrap();
    assert_eq!(
        quantize(&uniform, &Params::default(), &Control::default())
            .unwrap()
            .0
            .colors
            .len(),
        1
    );
}

#[test]
fn experimental_quantizers_keep_locks_and_are_deterministic() {
    for variant in [
        av_core::params::Quantizer::Kmeans,
        av_core::params::Quantizer::MedianCut,
        av_core::params::Quantizer::Wu,
    ] {
        let image = RgbaImage::new(
            16,
            16,
            (0..256)
                .map(|i| [i as u8, (255 - i) as u8, (i * 17) as u8, 255])
                .collect(),
        )
        .unwrap();
        let mut params = Params {
            colors: ColorCount::Fixed(8),
            palette_lock: vec![[255, 0, 0]],
            ..Params::default()
        };
        params.experimental.quantizer = variant;
        let a = quantize(&image, &params, &Control::default()).unwrap();
        assert_eq!(a, quantize(&image, &params, &Control::default()).unwrap());
        assert!(a.0.colors.iter().any(|c| c.locked && c.rgb == [255, 0, 0]));
        assert!(a.0.colors.len() <= 8);
        let mut first = Vec::new();
        for &id in &a.1.labels {
            if !first.contains(&id) {
                first.push(id);
            }
        }
        assert_eq!(first, (0..first.len() as u16).collect::<Vec<_>>());
    }
}

#[test]
fn merges_small_regions_and_preserves_transparent_sources() {
    let colors = [[0, 0, 0], [255, 255, 255], [250, 250, 250]];
    let palette = Palette {
        colors: colors
            .into_iter()
            .enumerate()
            .map(|(i, rgb)| PaletteColor {
                id: i as u16,
                rgb,
                oklab: oklab::from_srgb8(rgb),
                locked: false,
            })
            .collect(),
    };
    let ctl = Control::default();
    let p = Params {
        min_region_area: 2,
        ..Params::default()
    };
    let labels = LabelMap {
        w: 3,
        h: 3,
        labels: vec![1, 1, 1, 1, 2, 1, 1, 1, 1],
    };
    let map = label(&labels, None, &ctl).unwrap();
    let merged = merge_small(&map, &palette, &p, None, &ctl).unwrap();
    assert_eq!(merged.regions.len(), 1);
    assert_eq!(merged.regions[0].area, 9);
    let labels = LabelMap {
        w: 3,
        h: 3,
        labels: vec![1, 1, 1, 1, TRANSPARENT, 1, 1, 1, 1],
    };
    let map = label(&labels, None, &ctl).unwrap();
    let merged = merge_small(&map, &palette, &p, None, &ctl).unwrap();
    assert_eq!(merged.regions.len(), 2);
    let full = label(
        &LabelMap {
            w: 1,
            h: 1,
            labels: vec![0],
        },
        None,
        &ctl,
    )
    .unwrap();
    assert_eq!(
        merge_small(&full, &palette, &p, None, &ctl)
            .unwrap()
            .regions[0]
            .area,
        1
    );
}
