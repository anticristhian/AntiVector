//! Opaque canvas coverage must come from topology, never a bounding box guess.
use av_core::{
    assemble::{assemble, polygon_edges, Seg},
    pipeline::polygon_stages,
    Control, Params, RgbaImage, VectorDoc,
};
use av_export::{svg::SvgOptions, Format};

fn diagonal() -> VectorDoc {
    let image = RgbaImage::new(
        32,
        32,
        (0..32)
            .flat_map(|y| {
                (0..32).map(move |x| {
                    if x < y {
                        [24, 72, 120, 255]
                    } else {
                        [224, 168, 48, 255]
                    }
                })
            })
            .collect(),
    )
    .unwrap();
    let mut params = Params {
        min_region_area: 0,
        ..Params::default()
    };
    params.experimental.quality_v5 = false;
    let stages = polygon_stages(&image, &params, &Control::default()).unwrap();
    let mut fitted = polygon_edges(&stages.graph);
    for (edge, fitted) in stages.graph.edges.iter().zip(&mut fitted) {
        if !edge.on_frame {
            let end = edge.pts.last().unwrap();
            fitted.segs = vec![Seg::Line([f64::from(end.x), f64::from(end.y)])];
        }
    }
    assemble(
        &stages.merged,
        &stages.graph,
        &stages.palette,
        fitted,
        &Control::default(),
    )
    .unwrap()
}

#[test]
fn opaque_shared_diagonal_keeps_geometry_colors_and_ids_without_aa_transparency() {
    let doc = diagonal();
    let saved = doc.clone();
    let before_options = SvgOptions {
        opaque_canvas: false,
        ..SvgOptions::default()
    };
    let options = SvgOptions {
        opaque_canvas: true,
        ..before_options
    };
    let before = av_export::svg::to_string_with_options(&doc, &before_options).unwrap();
    let after = av_export::svg::to_string_with_options(&doc, &options).unwrap();
    let old_render = av_metrics::render::svg(&before, true).unwrap();
    let new_render = av_metrics::render::svg(&after, true).unwrap();
    assert!(old_render.px.iter().any(|pixel| pixel[3] < 255));
    assert!(new_render.px.iter().all(|pixel| pixel[3] == 255));
    assert_eq!(
        av_metrics::render::svg(&before, false).unwrap(),
        av_metrics::render::svg(&after, false).unwrap()
    );
    let paths = av_export::svg::to_paths(&doc, &options).unwrap();
    assert_eq!(paths.len(), doc.shapes.len());
    for (path, shape) in paths.iter().zip(&doc.shapes) {
        assert_eq!(path.region, Some(shape.region));
        assert_eq!(path.color, doc.palette.colors[shape.color as usize].rgb);
    }
    let png = av_export::export(&doc, Format::Png, &options).unwrap();
    let decoded = image::load_from_memory(&png).unwrap().into_rgba8();
    assert_eq!(
        decoded.as_raw(),
        &new_render.px.iter().flatten().copied().collect::<Vec<_>>()
    );
    let pdf = av_export::export(&doc, Format::Pdf, &options).unwrap();
    let parsed = lopdf::Document::load_mem(&pdf).unwrap();
    let content = parsed.get_page_content(parsed.get_pages()[&1]).unwrap();
    let operations = lopdf::content::Content::decode(&content)
        .unwrap()
        .operations;
    assert_eq!(
        operations.iter().filter(|op| op.operator == "f*").count(),
        doc.shapes.len()
    );
    let eps = String::from_utf8(av_export::export(&doc, Format::Eps, &options).unwrap()).unwrap();
    assert_eq!(eps.matches("\neofill\n").count(), doc.shapes.len());
    assert_eq!(
        av_export::export(&doc, Format::Dxf, &options).unwrap(),
        av_export::export(&doc, Format::Dxf, &before_options).unwrap()
    );
    assert_eq!(doc, saved);
}

#[test]
fn incomplete_frame_and_transparent_holes_never_receive_an_opaque_backdrop() {
    for keep in 0..2 {
        let mut doc = diagonal();
        doc.shapes = vec![doc.shapes[keep].clone()];
        let legacy = SvgOptions::default();
        let experimental = SvgOptions {
            opaque_canvas: true,
            ..legacy
        };
        assert_eq!(
            av_export::svg::to_string_with_options(&doc, &legacy).unwrap(),
            av_export::svg::to_string_with_options(&doc, &experimental).unwrap()
        );
    }
    let pixels = (0..16)
        .flat_map(|y| {
            (0..16).map(move |x| {
                if (5..11).contains(&x) && (5..11).contains(&y) {
                    [0; 4]
                } else {
                    [48, 96, 144, 255]
                }
            })
        })
        .collect();
    let image = RgbaImage::new(16, 16, pixels).unwrap();
    let mut params = Params {
        min_region_area: 0,
        ..Params::default()
    };
    params.experimental.quality_v5 = false;
    let doc = av_core::vectorize(&image, &params, &Control::default()).unwrap();
    let options = SvgOptions {
        opaque_canvas: true,
        ..SvgOptions::default()
    };
    let result = av_metrics::render::svg(
        &av_export::svg::to_string_with_options(&doc, &options).unwrap(),
        true,
    )
    .unwrap();
    assert_eq!(result.px[8 * 16 + 8][3], 0);
    let exact = SvgOptions {
        overlap: false,
        ..options
    };
    assert_eq!(
        av_export::svg::to_string_with_options(&diagonal(), &exact).unwrap(),
        av_export::svg::to_string_with_options(
            &diagonal(),
            &SvgOptions {
                opaque_canvas: false,
                ..exact
            }
        )
        .unwrap()
    );
}
