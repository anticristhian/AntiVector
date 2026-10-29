//! Analytic loops may move only an artificial seam; controls and real junctions stay exact.
use av_core::{assemble::Seg, Control, Params, RgbaImage};

fn source(name: &str) -> RgbaImage {
    let image = image::open(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/images")
            .join(format!("{name}.png")),
    )
    .unwrap()
    .into_rgba8();
    RgbaImage::new(
        image.width(),
        image.height(),
        image.pixels().map(|p| p.0).collect(),
    )
    .unwrap()
}

#[test]
fn real_junctions_are_fixed_and_isolated_circles_have_four_shared_arcs() {
    for name in [
        "circle_r50",
        "logo_original_1",
        "logo_original_3",
        "junction_y",
    ] {
        let mut p = Params {
            min_region_area: 4,
            ..Params::default()
        };
        p.experimental.circular_loops = true;
        let image = source(name);
        let stages = av_core::pipeline::polygon_stages(&image, &p, &Control::default()).unwrap();
        let fitted = av_core::fit::fit_edges_for_regions_at_scale(
            &stages.graph,
            &stages.merged,
            &stages.palette,
            &stages.params,
            stages.detection.is_pixel_art,
            stages.scale,
            &Control::default(),
        )
        .unwrap();
        let mut incidence = vec![0; stages.graph.junctions.len()];
        for edge in &stages.graph.edges {
            incidence[edge.start as usize] += 1;
            if edge.end != edge.start {
                incidence[edge.end as usize] += 1;
            }
        }
        let mut circular = 0;
        for (edge, fit) in stages.graph.edges.iter().zip(&fitted) {
            let first = edge.pts[0];
            let last = edge.pts.last().unwrap();
            if edge.start != edge.end || incidence[edge.start as usize] > 1 || edge.on_frame {
                assert_eq!(
                    fit.start,
                    [f64::from(first.x), f64::from(first.y)],
                    "real start {name}"
                );
                assert_eq!(
                    fit.segs.last().unwrap().end(),
                    [f64::from(last.x), f64::from(last.y)],
                    "real end {name}"
                );
            } else {
                assert_eq!(fit.segs.last().unwrap().end(), fit.start);
                if fit.segs.len() == 4 && fit.segs.iter().all(|s| matches!(s, Seg::Cubic(..))) {
                    circular += 1;
                }
            }
        }
        if name == "circle_r50" || name == "logo_original_1" {
            assert!(circular > 0, "{name}");
        }
    }
}

#[test]
fn circular_fidelity_improves_without_changing_non_circular_or_pixel_art_controls() {
    let mut p = Params {
        min_region_area: 4,
        ..Params::default()
    };
    for name in [
        "circle_r50",
        "ellipse",
        "star5",
        "circle_noaa",
        "pixel_robot",
    ] {
        let image = source(name);
        p.experimental.circular_loops = false;
        let old = av_core::vectorize(&image, &p, &Control::default()).unwrap();
        let old = av_export::svg::to_string(&old).unwrap();
        p.experimental.circular_loops = true;
        let new = av_core::vectorize(&image, &p, &Control::default()).unwrap();
        let new = av_export::svg::to_string(&new).unwrap();
        if name != "circle_r50" {
            assert_eq!(old, new, "control {name}");
            continue;
        }
        let before = av_metrics::compare(
            &image,
            &av_metrics::render::svg(&old, true).unwrap(),
            &av_metrics::render::svg(&old, false).unwrap(),
            128,
        )
        .unwrap();
        let after = av_metrics::compare(
            &image,
            &av_metrics::render::svg(&new, true).unwrap(),
            &av_metrics::render::svg(&new, false).unwrap(),
            128,
        )
        .unwrap();
        assert!(after.ssim > before.ssim);
        assert!(after.delta_e_mean < before.delta_e_mean);
        assert_eq!(after.gap_pixels, 0);
        let truth = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/gt/circle_r50.svg"),
        )
        .unwrap();
        let distance = av_metrics::geometry::hausdorff(
            &av_metrics::geometry::inspect(&new).unwrap(),
            &av_metrics::geometry::inspect(&truth).unwrap(),
        )
        .unwrap();
        assert!(distance <= 0.5);
    }
}

#[test]
fn circular_candidate_is_thread_deterministic_and_keeps_node_monotonicity() {
    let options = av_export::svg::SvgOptions {
        opaque_canvas: true,
        ..Default::default()
    };
    for name in ["circle_r50", "concentric", "logo_compass"] {
        let image = source(name);
        let mut params = Params {
            min_region_area: 4,
            ..Params::default()
        };
        params.experimental.circular_loops = true;
        let render = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let doc = av_core::vectorize(&image, &params, &Control::default()).unwrap();
                    av_export::svg::to_string_with_options(&doc, &options).unwrap()
                })
        };
        assert_eq!(render(1), render(4), "thread determinism {name}");
        let mut previous = usize::MAX;
        for tolerance in [0.1, 0.3, 0.6, 0.9, 1.5, 3.0] {
            params.fit_tolerance_px = tolerance;
            let doc = av_core::vectorize(&image, &params, &Control::default()).unwrap();
            let nodes = av_export::statistics(&doc, av_export::Format::Svg, &options)
                .unwrap()
                .nodes;
            assert!(
                nodes <= previous,
                "node monotonicity {name} at {tolerance}: {previous}->{nodes}"
            );
            previous = nodes;
        }
    }
}
