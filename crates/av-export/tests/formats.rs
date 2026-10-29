//! Independent parsers and source-resolution renders for the entire corpus.
use av_core::{
    assemble::{FittedEdge, Seg},
    Control, Params, RgbaImage,
};
use av_export::{svg::SvgOptions, Format};
use std::{collections::BTreeSet, path::PathBuf};

#[test]
fn all_formats_parse_and_preserve_the_entire_corpus() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = root.join("target/f6-export-tests");
    std::fs::create_dir_all(&out).unwrap();
    let entries: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(root.join("corpus/manifest.json")).unwrap()).unwrap();
    let mut results = Vec::new();
    for entry in &entries {
        let id = entry["id"].as_str().unwrap();
        if entry["expected_decode_error"].as_bool() == Some(true) {
            assert!(
                image::open(root.join("corpus").join(entry["file"].as_str().unwrap())).is_err(),
                "corrupt input accepted: {id}"
            );
            results.push(serde_json::json!({"id":id,"expected_decode_error":true,"passed":true}));
            continue;
        }
        let source = image::open(root.join("corpus").join(entry["file"].as_str().unwrap()))
            .unwrap()
            .to_rgba8();
        let image = RgbaImage::new(
            source.width(),
            source.height(),
            source.pixels().map(|p| p.0).collect(),
        )
        .unwrap();
        let mut params =
            av_core::params::resolve_defaults(av_core::Mode::Logo, &av_core::detect(&image));
        params.min_region_area =
            av_core::params::default_min_region_area(params.detail, image.w, image.h);
        params.experimental.circular_loops = true;
        let doc = av_core::vectorize(&image, &params, &Control::default()).unwrap();
        let before = doc.clone();
        // Validate the opt-in candidate across every source and format; defaults
        // remain legacy until the separate human gate has passed.
        let options = SvgOptions {
            opaque_canvas: true,
            ..SvgOptions::default()
        };
        let svg =
            String::from_utf8(av_export::export(&doc, Format::Svg, &options).unwrap()).unwrap();
        assert!(svg.contains(&format!("viewBox=\"0 0 {} {}\"", image.w, image.h)));
        assert!(!svg.contains("NaN"));
        resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
        let internal = av_metrics::render::svg(&svg, true).unwrap();
        let png = av_export::export(&doc, Format::Png, &options).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        let raster = RgbaImage::new(
            decoded.width(),
            decoded.height(),
            decoded.pixels().map(|p| p.0).collect(),
        )
        .unwrap();
        assert_eq!(internal, raster, "PNG/SVG render {id}");
        let pdf = av_export::export(&doc, Format::Pdf, &options).unwrap();
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        let page = parsed.get_pages()[&1];
        let content = parsed.get_page_content(page).unwrap();
        let ops = lopdf::content::Content::decode(&content)
            .unwrap()
            .operations;
        assert_eq!(
            av_export::statistics(&doc, Format::Pdf, &options)
                .unwrap()
                .paths,
            ops.iter().filter(|op| op.operator == "f*").count()
        );
        assert_eq!(
            ops.iter().filter(|op| op.operator == "f*").count(),
            doc.shapes.len(),
            "PDF paths {id}"
        );
        assert!(
            !ops.iter().any(|op| op.operator == "Do"),
            "PDF embeds raster"
        );
        let eps =
            String::from_utf8(av_export::export(&doc, Format::Eps, &options).unwrap()).unwrap();
        assert!(eps.starts_with("%!PS-Adobe-3.0 EPSF-3.0\n"));
        assert!(eps.contains("%%LanguageLevel: 2\n"));
        assert!(eps.ends_with("%%EOF\n"));
        assert!(eps.contains(&format!(
            "%%BoundingBox: 0 0 {:.0} {:.0}\n",
            (doc.width * 0.75).ceil(),
            (doc.height * 0.75).ceil()
        )));
        assert_eq!(eps.matches("\neofill\n").count(), doc.shapes.len());
        let dxf = av_export::export(&doc, Format::Dxf, &options).unwrap();
        let drawing = dxf::Drawing::load(&mut std::io::Cursor::new(&dxf)).unwrap();
        assert_eq!(drawing.header.version, dxf::enums::AcadVersion::R12);
        let used: BTreeSet<_> = doc
            .shapes
            .iter()
            .flat_map(|s| s.loops.iter().flatten().map(|&(id, _)| id))
            .collect();
        assert_eq!(drawing.entities().count(), used.len());
        assert_eq!(
            av_export::statistics(&doc, Format::Dxf, &options)
                .unwrap()
                .paths,
            drawing.entities().count()
        );
        for entity in drawing.entities() {
            let dxf::entities::EntityType::Polyline(p) = &entity.specific else {
                panic!("non R12 polyline")
            };
            assert!(p.vertices().count() >= 2);
        }
        for (ext, bytes) in [
            ("svg", svg.as_bytes()),
            ("png", &png),
            ("pdf", &pdf),
            ("eps", eps.as_bytes()),
            ("dxf", &dxf),
        ] {
            std::fs::write(out.join(format!("{id}.{ext}")), bytes).unwrap();
        }
        assert_eq!(doc, before, "export mutated document {id}");
        results.push(serde_json::json!({"id":id,"formats":5,"svg_png_identical":true,"pdf_fills":doc.shapes.len(),"dxf_shared_boundaries":used.len()}));
    }
    assert_eq!(results.len(), entries.len());
    std::fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&results).unwrap(),
    )
    .unwrap();
}

#[test]
fn precision_color_grouping_and_invalid_inputs() {
    let mut pixels = vec![[255; 4]; 20 * 20];
    for y in 2..6 {
        for x in 2..6 {
            pixels[y * 20 + x] = [0, 0, 0, 255];
        }
    }
    for y in 12..16 {
        for x in 12..16 {
            pixels[y * 20 + x] = [0, 0, 0, 255];
        }
    }
    let image = RgbaImage::new(20, 20, pixels).unwrap();
    let mut p = Params {
        min_region_area: 0,
        ..Params::default()
    };
    p.experimental.force_pixel_art = true;
    let doc = av_core::vectorize(&image, &p, &Control::default()).unwrap();
    let exact = SvgOptions {
        overlap: false,
        ..SvgOptions::default()
    };
    let grouped = SvgOptions {
        group_by_color: true,
        ..exact
    };
    let a = av_export::svg::to_string_with_options(&doc, &exact).unwrap();
    let b = av_export::svg::to_string_with_options(&doc, &grouped).unwrap();
    assert_eq!(b.matches("<path ").count(), 2);
    assert_eq!(
        av_metrics::render::svg(&a, false).unwrap(),
        av_metrics::render::svg(&b, false).unwrap()
    );
    let mut doc = doc;
    doc.edges[0].start[0] += 0.1234567;
    // Precision is exercised on an independent closed fractional contour.
    let edge = FittedEdge {
        id: 0,
        start: [1.1234567, 1.1234567],
        segs: vec![
            Seg::Line([5.9876543, 1.1234567]),
            Seg::Line([5.9876543, 5.9876543]),
            Seg::Line([1.1234567, 1.1234567]),
        ],
    };
    doc.edges = vec![edge];
    doc.shapes.truncate(1);
    doc.shapes[0].loops = vec![vec![(0, false)]];
    doc.shapes[0].hole_regions.clear();
    for precision in [0, 2, 4, 8] {
        let opts = SvgOptions { precision, ..exact };
        let svg = av_export::svg::to_string_with_options(&doc, &opts).unwrap();
        assert!(!svg.contains("-0 "));
        resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
        let data = svg
            .split(" d=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        for token in data.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
            if let Some((_, decimals)) = token.split_once('.') {
                assert!(decimals.len() <= usize::from(precision));
            }
        }
    }
    assert!(SvgOptions {
        precision: 9,
        ..exact
    }
    .validate()
    .is_err());
    assert!(SvgOptions {
        group_by_color: true,
        ..Default::default()
    }
    .validate()
    .is_err());
    doc.edges[0].start[0] = f64::NAN;
    for format in [
        Format::Svg,
        Format::Pdf,
        Format::Eps,
        Format::Dxf,
        Format::Png,
    ] {
        assert!(av_export::export(&doc, format, &exact).is_err());
    }
}

#[test]
fn dxf_subdivision_stays_within_point_one_pixels() {
    let edge = FittedEdge {
        id: 0,
        start: [0.0, 0.0],
        segs: vec![Seg::Cubic([60.0, 120.0], [-30.0, -100.0], [80.0, 10.0])],
    };
    let flat = av_export::dxf::flatten_edge(&edge).unwrap();
    assert_eq!(flat.first(), Some(&edge.start));
    assert_eq!(flat.last(), Some(&[80.0, 10.0]));
    let mut maximum = 0_f64;
    for i in 0..=10000 {
        let t = f64::from(i) / 10000.0;
        let s = 1.0 - t;
        let p = [
            3.0 * s * s * t * 60.0 + 3.0 * s * t * t * (-30.0) + t * t * t * 80.0,
            3.0 * s * s * t * 120.0 + 3.0 * s * t * t * (-100.0) + t * t * t * 10.0,
        ];
        let dist = flat
            .windows(2)
            .map(|pair| {
                let a = pair[0];
                let b = pair[1];
                let v = [b[0] - a[0], b[1] - a[1]];
                let d = v[0] * v[0] + v[1] * v[1];
                let q = if d == 0.0 {
                    0.0
                } else {
                    (((p[0] - a[0]) * v[0] + (p[1] - a[1]) * v[1]) / d).clamp(0.0, 1.0)
                };
                (p[0] - a[0] - q * v[0]).hypot(p[1] - a[1] - q * v[1])
            })
            .fold(f64::INFINITY, f64::min);
        maximum = maximum.max(dist);
    }
    println!(
        "DXF maximum sampled deviation={maximum:.12}px, vertices={}",
        flat.len()
    );
    assert!(maximum <= 0.1);
}

#[test]
fn deterministic_vector_format_snapshots() {
    let mut pixels = vec![[255, 0, 0, 255]; 9];
    pixels[4] = [0, 0, 255, 255];
    let image = RgbaImage::new(3, 3, pixels).unwrap();
    let p = Params {
        min_region_area: 0,
        ..Params::default()
    };
    let doc = av_core::vectorize(&image, &p, &Control::default()).unwrap();
    let options = SvgOptions::default();
    insta::assert_snapshot!("eps", av_export::eps::to_string(&doc, &options).unwrap());
    insta::assert_snapshot!("dxf_r12", av_export::dxf::to_string(&doc).unwrap());
    let parsed =
        lopdf::Document::load_mem(&av_export::pdf::to_bytes(&doc, &options).unwrap()).unwrap();
    insta::assert_snapshot!(
        "pdf_content",
        String::from_utf8(parsed.get_page_content(parsed.get_pages()[&1]).unwrap()).unwrap()
    );
}
