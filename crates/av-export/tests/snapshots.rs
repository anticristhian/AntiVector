//! Historical polygon/F4 controls and the currently validated quality defaults.

#[test]
fn synthetic_polygon_snapshots() {
    snapshots(false, false);
}

#[test]
fn synthetic_curve_snapshots() {
    snapshots(true, false);
}

#[test]
fn synthetic_quality_snapshots() {
    snapshots(true, true);
}

fn snapshots(curves: bool, quality: bool) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/images");
    for name in [
        "circle_r50",
        "square_rot30",
        "star5",
        "ring",
        "concentric",
        "junction_t",
        "junction_y",
        "line_1px",
        "line_2px",
        "checker",
    ] {
        let image = image::open(root.join(format!("{name}.png")))
            .unwrap()
            .into_rgba8();
        let raster = av_core::RgbaImage::new(
            image.width(),
            image.height(),
            image.pixels().map(|p| p.0).collect(),
        )
        .unwrap();
        let mut params = av_core::Params {
            min_region_area: av_core::params::default_min_region_area(0.5, raster.w, raster.h),
            ..Default::default()
        };
        params.experimental.curve_fit = curves;
        params.experimental.quality_v5 = quality;
        if !quality {
            params.experimental.circular_loops = false;
        }
        params.experimental.corner_recovery_radius = 1.5;
        let doc = av_core::vectorize(&raster, &params, &Default::default()).unwrap();
        let svg = av_export::svg::to_string_with_options(
            &doc,
            &av_export::svg::SvgOptions {
                overlap: quality,
                ..Default::default()
            },
        )
        .unwrap();
        let snapshot_name = if quality {
            format!("quality_{name}")
        } else if curves {
            format!("curves_{name}")
        } else {
            name.to_string()
        };
        insta::assert_snapshot!(snapshot_name, svg);
    }
}
