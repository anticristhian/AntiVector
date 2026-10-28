use av_core::{
    assemble::{assemble, Seg},
    boundary::graph::BoundaryGraph,
    color::Palette,
    pipeline::polygon_stages,
    segment::RegionMap,
    Control, Params, RgbaImage,
};

fn fixture(name: &str) -> (RgbaImage, RegionMap, BoundaryGraph, Palette, Params) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/images");
    let image = image::open(root.join(format!("{name}.png")))
        .unwrap()
        .into_rgba8();
    let image = RgbaImage::new(
        image.width(),
        image.height(),
        image.pixels().map(|p| p.0).collect(),
    )
    .unwrap();
    let mut params = Params {
        min_region_area: 4,
        ..Params::default()
    };
    params.experimental.curve_fit = true;
    params.experimental.quality_v5 = false;
    params.experimental.corner_recovery_radius = 1.5;
    let stages = polygon_stages(&image, &params, &Control::default()).unwrap();
    let av_core::pipeline::PolygonStages {
        merged,
        graph,
        palette,
        preprocessed,
        ..
    } = stages;
    drop(preprocessed);
    (image, merged, graph, palette, params)
}

#[test]
fn circle_square_star_and_fixed_junctions() {
    for (name, count) in [("square_rot30", 4), ("star5", 10), ("circle_r50", 8)] {
        let (_, map, graph, palette, params) = fixture(name);
        let edges =
            av_core::stages::fit_edges(&graph, &params, false, &Control::default()).unwrap();
        let lengths: Vec<_> = graph
            .edges
            .iter()
            .filter(|edge| !edge.on_frame)
            .map(|edge| edges[edge.id as usize].segs.len())
            .collect();
        println!("{name}: non-frame segment counts {lengths:?}");
        if name == "circle_r50" {
            assert!(lengths.iter().all(|&n| n <= count));
        } else if name == "square_rot30" {
            assert_eq!(lengths, vec![count]);
            assert!(graph
                .edges
                .iter()
                .filter(|edge| !edge.on_frame)
                .all(|edge| edges[edge.id as usize]
                    .segs
                    .iter()
                    .all(|seg| matches!(seg, Seg::Line(_)))));
        }
        for (edge, fitted) in graph.edges.iter().zip(&edges) {
            assert_eq!(
                fitted.start,
                [f64::from(edge.pts[0].x), f64::from(edge.pts[0].y)]
            );
            let end = edge.pts.last().unwrap();
            assert_eq!(
                fitted.segs.last().unwrap().end(),
                [f64::from(end.x), f64::from(end.y)]
            );
        }
        let doc = assemble(&map, &graph, &palette, edges, &Control::default()).unwrap();
        let svg = av_export::svg::to_string(&doc).unwrap();
        if name == "star5" {
            let mut foreground = doc.clone();
            let region = map.regions.iter().min_by_key(|r| r.area).unwrap().id;
            foreground.shapes.retain(|s| s.region == region);
            let geometry =
                av_metrics::geometry::inspect(&av_export::svg::to_string(&foreground).unwrap())
                    .unwrap();
            println!(
                "star sharp corners {}: {:?}",
                geometry.corners.len(),
                geometry.corners
            );
            assert_eq!(geometry.corners.len(), 10);
        }
        if name == "circle_r50" {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/gt");
            let gt = std::fs::read_to_string(root.join("circle_r50.svg")).unwrap();
            let distance = av_metrics::geometry::hausdorff(
                &av_metrics::geometry::inspect(&svg).unwrap(),
                &av_metrics::geometry::inspect(&gt).unwrap(),
            )
            .unwrap();
            println!("circle r50 Hausdorff = {distance:.8}");
            assert!(distance <= 0.5);
        }
    }
}

#[test]
fn deterministic_threads_and_monotonic_tolerance() {
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
        let (image, map, graph, palette, mut params) = fixture(name);
        let pixel_art = av_core::detect(&image).is_pixel_art;
        let mut previous = usize::MAX;
        for tolerance in [0.3, 0.9, 1.5, 3.0] {
            params.fit_tolerance_px = tolerance;
            let result =
                av_core::stages::fit_edges(&graph, &params, pixel_art, &Control::default())
                    .unwrap();
            let doc = assemble(&map, &graph, &palette, result, &Control::default()).unwrap();
            assert!(
                doc.stats.nodes <= previous,
                "{name} tolerance {tolerance}: {} > {previous}",
                doc.stats.nodes
            );
            previous = doc.stats.nodes;
        }
        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    av_export::svg::to_string(
                        &av_core::vectorize(&image, &params, &Control::default()).unwrap(),
                    )
                    .unwrap()
                })
        };
        assert_eq!(run(1), run(4), "{name}");
    }
}
