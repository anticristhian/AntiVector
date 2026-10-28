//! Exact topology and rendering checks: the SVG must equal the label raster.

use av_core::{
    assemble::{assemble, polygon_edges},
    boundary::{
        extract::extract_boundaries,
        graph::{signed_area2, IPoint},
    },
    color::{oklab, LabelMap, Palette, PaletteColor, TRANSPARENT},
    segment::label::label,
    Control,
};
use proptest::prelude::*;
use std::collections::BTreeSet;

fn palette() -> Palette {
    Palette {
        colors: [
            [0, 0, 0],
            [255, 255, 255],
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [200, 100, 0],
        ]
        .into_iter()
        .enumerate()
        .map(|(i, rgb)| PaletteColor {
            id: i as u16,
            rgb,
            oklab: oklab::from_srgb8(rgb),
            locked: false,
        })
        .collect(),
    }
}

fn check(w: u32, h: u32, labels: Vec<u16>) {
    let ctl = Control::default();
    let colors = palette();
    let labels = LabelMap { w, h, labels };
    let map = label(&labels, None, &ctl).unwrap();
    let graph = extract_boundaries(&map, &ctl).unwrap();
    let mut usage = vec![0; graph.edges.len()];
    let mut seen = BTreeSet::new();
    for edge in &graph.edges {
        assert_eq!(edge.pts[0], graph.junctions[edge.start as usize].point);
        assert_eq!(
            *edge.pts.last().unwrap(),
            graph.junctions[edge.end as usize].point
        );
        for segment in edge.pts.windows(2) {
            let a = segment[0];
            let b = segment[1];
            assert_eq!(a.x.abs_diff(b.x) + a.y.abs_diff(b.y), 1);
            assert!(seen.insert((a.min(b), a.max(b))), "crack reused");
        }
    }
    let mut expected = BTreeSet::new();
    let at = |x: u32, y: u32| map.region_of[(y * w + x) as usize];
    for y in 0..=h {
        for x in 0..w {
            if y == 0 || y == h || at(x, y - 1) != at(x, y) {
                expected.insert((IPoint { x, y }, IPoint { x: x + 1, y }));
            }
        }
    }
    for y in 0..h {
        for x in 0..=w {
            if x == 0 || x == w || at(x - 1, y) != at(x, y) {
                expected.insert((IPoint { x, y }, IPoint { x, y: y + 1 }));
            }
        }
    }
    assert_eq!(seen, expected, "every crack must be owned once");
    for (region, loops) in graph.region_loops.iter().enumerate() {
        let area: i64 = loops
            .iter()
            .map(|path| {
                for &(edge, _) in path {
                    usage[edge as usize] += 1;
                }
                let points = graph.loop_points(path);
                assert_eq!(points.first(), points.last());
                signed_area2(&points)
            })
            .sum();
        assert_eq!(area, i64::from(map.regions[region].area) * 2, "region area");
        assert!(signed_area2(&graph.loop_points(&loops[0])) > 0);
        for hole in &loops[1..] {
            assert!(signed_area2(&graph.loop_points(hole)) < 0);
        }
    }
    for (edge, count) in graph.edges.iter().zip(usage) {
        assert_eq!(count, if edge.on_frame { 1 } else { 2 });
    }
    let doc = assemble(&map, &graph, &colors, polygon_edges(&graph), &ctl).unwrap();
    let svg = av_export::svg::to_string(&doc).unwrap();
    let options = resvg::usvg::Options {
        shape_rendering: resvg::usvg::ShapeRendering::CrispEdges,
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(&svg, &options).unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).unwrap();
    pixmap.fill(resvg::tiny_skia::Color::from_rgba8(255, 0, 255, 255));
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let mut gap = 0;
    for (&id, pixel) in labels.labels.iter().zip(pixmap.pixels()) {
        let expected = if id == TRANSPARENT {
            [255, 0, 255]
        } else {
            colors.colors[id as usize].rgb
        };
        let actual = [pixel.red(), pixel.green(), pixel.blue()];
        if id != TRANSPARENT && actual == [255, 0, 255] {
            gap += 1;
        }
        assert_eq!(actual, expected, "SVG must match quantized raster");
    }
    assert_eq!(gap, 0);
}

#[test]
fn explicit_topology_cases() {
    check(2, 2, vec![0, 1, 1, 0]); // diagonal junction
    check(3, 3, vec![0, 0, 0, 0, 1, 0, 0, 0, 0]); // island/hole
    check(3, 3, vec![0, 1, 0, 1, 0, 1, 0, 1, 0]); // diagonal contacts
    check(3, 3, vec![0, 0, 1, 0, 1, 1, 0, 0, 0]); // one region touches itself
    check(1, 1, vec![TRANSPARENT]);
    check(1, 10, vec![0, 1, 0, 1, 1, 0, 0, 1, 0, 1]);
    let mut nested = vec![0; 49];
    for y in 1..6 {
        for x in 1..6 {
            nested[y * 7 + x] = 1;
        }
    }
    for y in 2..5 {
        for x in 2..5 {
            nested[y * 7 + x] = 2;
        }
    }
    nested[24] = TRANSPARENT;
    check(7, 7, nested);
}

proptest! {
    #![proptest_config(ProptestConfig {cases:512,rng_seed:proptest::test_runner::RngSeed::Fixed(0x41565432),..ProptestConfig::default()})]
    #[test]
    fn exact_coverage_random_labels((w,h,n,bytes) in (1_u32..=40,1_u32..=40,2_u16..=6).prop_flat_map(|(w,h,n)| (Just(w),Just(h),Just(n),proptest::collection::vec(any::<u8>(),(w*h) as usize)))) {
        let labels:Vec<u16> = bytes.into_iter().map(|b| if b%13==0 {TRANSPARENT} else {u16::from(b)%n}).collect();
        check(w,h,labels.clone());
        let ctl = Control::default();
        let map = label(&LabelMap {w,h,labels},None,&ctl).unwrap();
        let params = av_core::Params {min_region_area:4,..Default::default()};
        let merged = av_core::stages::merge_small(&map,&palette(),&params,None,&ctl).unwrap();
        check(w,h,merged.region_of.iter().map(|&id| merged.regions[id as usize].color).collect());
    }
}
