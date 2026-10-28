//! Polygon references plus the conservatively validated F4 curve configuration.

#[test]
fn synthetic_polygon_snapshots() {
    snapshots(false);
}

#[test]
fn synthetic_curve_snapshots() {
    snapshots(true);
}

fn snapshots(curves: bool) {
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
        params.experimental.corner_recovery_radius = 1.5;
        let doc = av_core::vectorize(&raster, &params, &Default::default()).unwrap();
        let svg = av_export::svg::to_string(&doc).unwrap();
        let snapshot_name = if curves {
            format!("curves_{name}")
        } else {
            name.to_string()
        };
        insta::assert_snapshot!(snapshot_name, svg);
    }
}
