//! Sequences of edits preserve exact label coverage and shared-boundary topology.
use av_core::{
    assemble::{assemble, polygon_edges},
    color::{oklab, LabelMap, Palette, PaletteColor, TRANSPARENT},
    edit::{apply, EditOp},
    segment::{label::label, RegionMap},
    Control,
};
use proptest::prelude::*;
fn palette() -> Palette {
    Palette {
        colors: [[20, 40, 80], [210, 60, 30], [240, 200, 50], [60, 160, 110]]
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
fn verify(map: &RegionMap, palette: &Palette) {
    let ctl = Control::default();
    let graph = av_core::stages::extract_boundaries(map, &ctl).unwrap();
    let mut usage = vec![0; graph.edges.len()];
    for loops in &graph.region_loops {
        for path in loops {
            for &(edge, _) in path {
                usage[edge as usize] += 1;
            }
        }
    }
    for (edge, count) in graph.edges.iter().zip(usage) {
        assert_eq!(count, if edge.on_frame { 1 } else { 2 });
    }
    let doc = assemble(map, &graph, palette, polygon_edges(&graph), &ctl).unwrap();
    let export_options = av_export::svg::SvgOptions {
        opaque_canvas: true,
        ..Default::default()
    };
    let svg = av_export::svg::to_string_with_options(&doc, &export_options).unwrap();
    let options = resvg::usvg::Options {
        shape_rendering: resvg::usvg::ShapeRendering::CrispEdges,
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(&svg, &options).unwrap();
    let mut pixels = resvg::tiny_skia::Pixmap::new(map.w, map.h).unwrap();
    pixels.fill(resvg::tiny_skia::Color::from_rgba8(255, 0, 255, 255));
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    for (&region, pixel) in map.region_of.iter().zip(pixels.pixels()) {
        let color = map.regions[region as usize].color;
        let expected = if color == TRANSPARENT {
            [255, 0, 255]
        } else {
            palette.colors[color as usize].rgb
        };
        assert_eq!(
            [pixel.red(), pixel.green(), pixel.blue()],
            expected,
            "edit introduced a gap or changed an unrelated pixel"
        );
    }
    let mut params = av_core::Params {
        min_region_area: 2,
        ..Default::default()
    };
    params.experimental.circular_loops = true;
    let edges =
        av_core::fit::fit_edges_for_regions(&graph, map, palette, &params, false, &ctl).unwrap();
    let fitted = assemble(map, &graph, palette, edges, &ctl).unwrap();
    let tree = resvg::usvg::Tree::from_str(
        &av_export::svg::to_string_with_options(&fitted, &export_options).unwrap(),
        &options,
    )
    .unwrap();
    let mut coverage = resvg::tiny_skia::Pixmap::new(map.w, map.h).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut coverage.as_mut(),
    );
    for (&region, pixel) in map.region_of.iter().zip(coverage.pixels()) {
        assert_eq!(
            pixel.alpha(),
            if map.regions[region as usize].color == TRANSPARENT {
                0
            } else {
                255
            },
            "fitted edit coverage changed"
        );
    }
}
#[test]
fn aliases_transparency_and_invalid_operations_are_safe() {
    let ctl = Control::default();
    let colors = palette();
    let map = label(
        &LabelMap {
            w: 3,
            h: 1,
            labels: vec![0, 1, 2],
        },
        None,
        &ctl,
    )
    .unwrap();
    assert!(apply(
        &map,
        &colors,
        &EditOp::MergeRegion {
            region: 0,
            into: Some(2)
        },
        None,
        &ctl
    )
    .is_err());
    let edited = apply(
        &map,
        &colors,
        &EditOp::RecolorPalette {
            color: 1,
            rgb: colors.colors[0].rgb,
        },
        None,
        &ctl,
    )
    .unwrap();
    assert_eq!(edited.map.regions.len(), 2);
    verify(&edited.map, &edited.palette);
    let edited = apply(
        &edited.map,
        &edited.palette,
        &EditOp::DeletePalette { color: 0 },
        None,
        &ctl,
    )
    .unwrap();
    verify(&edited.map, &edited.palette);
    let edited = apply(
        &edited.map,
        &edited.palette,
        &EditOp::DeletePalette { color: 0 },
        None,
        &ctl,
    )
    .unwrap();
    assert!(edited.palette.colors.is_empty());
    verify(&edited.map, &edited.palette);
    assert_eq!(map.region_of, vec![0, 1, 2]);
    assert_eq!(colors, palette());
}
proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x41564538),..ProptestConfig::default()})]
    #[test]
    fn random_edit_sequences_keep_zero_gaps((w,h,bytes,ops) in (1_u32..=24,1_u32..=24).prop_flat_map(|(w,h)|
        (Just(w),Just(h),proptest::collection::vec(any::<u8>(),(w*h) as usize),proptest::collection::vec((any::<u8>(),any::<u8>()),1..=12)))) {
        let ctl=Control::default(); let mut colors=palette();
        let mut map=label(&LabelMap{w,h,labels:bytes.iter().map(|b|if b%13==0{TRANSPARENT}else{u16::from(b%4)}).collect()},None,&ctl).unwrap();
        for (kind,value) in ops {
            let region=u32::from(value)%map.regions.len() as u32;
            let color=if colors.colors.is_empty(){None}else{Some(u16::from(value)%colors.colors.len() as u16)};
            let op=match(kind%6,color) {
                (0,Some(from))=>EditOp::MergePalette{from,into:(from+1)%colors.colors.len() as u16},
                (1,Some(color))=>EditOp::RecolorPalette{color,rgb:[value,100,170]},
                (2,Some(color))=>EditOp::DeletePalette{color},
                (3,Some(color))=>EditOp::LockColor{color,locked:value%2==0},
                (4,_)=>EditOp::MergeRegion{region,into:None},
                _=>EditOp::RemoveRegion{region}
            };
            let edited=if kind%6==5 {
                let mask:Vec<bool>=map.region_of.iter().enumerate().map(|(i,&id)|id==region && (i+usize::from(value))%3!=0).collect();
                av_core::edit::apply_mask(&map,&colors,&mask,TRANSPARENT,None,&ctl)
            }else{apply(&map,&colors,&op,None,&ctl)};
            if let Ok(edited)=edited {map=edited.map; colors=edited.palette;}
            verify(&map,&colors);
        }
    }
}
