use av_core::{ColorCount, Control, Params, RgbaImage};
use av_export::svg::{to_string_with_options, SvgOptions};

#[test]
fn quality_pipeline_svg_is_identical_with_one_and_four_threads() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/images");
    for name in ["logo_original_1", "concentric", "junction_y", "pixel_robot"] {
        let raster = image::open(root.join(format!("{name}.png")))
            .unwrap()
            .to_rgba8();
        let image = RgbaImage::new(
            raster.width(),
            raster.height(),
            raster.pixels().map(|p| p.0).collect(),
        )
        .unwrap();
        let p = Params {
            min_region_area: 4,
            ..Params::default()
        };
        let results: Vec<_> = [1, 4]
            .into_iter()
            .map(|threads| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        av_export::svg::to_string(
                            &av_core::vectorize(&image, &p, &Control::default()).unwrap(),
                        )
                        .unwrap()
                    })
            })
            .collect();
        assert_eq!(results[0], results[1], "{name}");
    }
}

fn verify(image: RgbaImage, colors: u16) -> (String, String) {
    let mut params = Params {
        colors: ColorCount::Fixed(colors),
        min_region_area: 0,
        ..Params::default()
    };
    // These exact-color grid fixtures exercise composition, without interpolation.
    params.experimental.force_pixel_art = true;
    let doc = av_core::vectorize(&image, &params, &Control::default()).unwrap();
    let exact = to_string_with_options(
        &doc,
        &SvgOptions {
            overlap: false,
            ..Default::default()
        },
    )
    .unwrap();
    let overlap = to_string_with_options(
        &doc,
        &SvgOptions {
            overlap: true,
            ..Default::default()
        },
    )
    .unwrap();
    let a = av_metrics::render::svg(&exact, false).unwrap();
    let b = av_metrics::render::svg(&overlap, false).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.px, image.px);
    (exact, overlap)
}

#[test]
fn omits_only_holes_filled_by_later_opaque_shapes() {
    let mut px = vec![[255; 4]; 49];
    for y in 1..6 {
        for x in 1..6 {
            px[y * 7 + x] = [0, 0, 255, 255];
        }
    }
    for y in 2..5 {
        for x in 2..5 {
            px[y * 7 + x] = [255, 128, 0, 255];
        }
    }
    let (exact, overlap) = verify(RgbaImage::new(7, 7, px).unwrap(), 3);
    assert!(overlap.matches('M').count() < exact.matches('M').count());
}

#[test]
fn transparent_holes_and_large_earlier_descendants_are_preserved() {
    let mut px = vec![[0, 0, 255, 255]; 25];
    px[12] = [0; 4];
    let (exact, overlap) = verify(RgbaImage::new(5, 5, px).unwrap(), 2);
    assert_eq!(exact, overlap);
    let mut px = vec![[0, 0, 255, 255]; 13 * 13];
    for y in 1..12 {
        for x in 1..12 {
            px[y * 13 + x] = [0, 255, 0, 255];
        }
    }
    for y in 2..11 {
        for x in 2..11 {
            px[y * 13 + x] = [255, 128, 0, 255];
        }
    }
    verify(RgbaImage::new(13, 13, px).unwrap(), 3);
}
