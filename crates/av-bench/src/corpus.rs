//! Original, deterministic synthetic fixtures and ground-truth SVGs; no external assets.

use crate::Result;
use av_core::RgbaImage;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub file: String,
    pub category: String,
    pub source: String,
    pub license: String,
    pub expected_colors: Option<u16>,
    pub notes: String,
    pub ground_truth: Option<String>,
    #[serde(default)]
    pub corners: Vec<[f64; 2]>,
}

pub fn load(root: &Path) -> Result<Vec<Entry>> {
    Ok(serde_json::from_slice(&fs::read(
        root.join("manifest.json"),
    )?)?)
}

pub fn read(root: &Path, entry: &Entry) -> Result<RgbaImage> {
    let rgba = image::open(root.join(&entry.file))?.into_rgba8();
    Ok(RgbaImage::new(
        rgba.width(),
        rgba.height(),
        rgba.pixels().map(|p| p.0).collect(),
    )?)
}

pub fn save_png(path: &Path, image: &RgbaImage) -> Result<()> {
    let bytes: Vec<_> = image.px.iter().flatten().copied().collect();
    image::save_buffer_with_format(
        path,
        &bytes,
        image.w,
        image.h,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )?;
    Ok(())
}

fn svg(body: &str) -> String {
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"128\" height=\"128\" viewBox=\"0 0 128 128\">{body}</svg>\n")
}
fn background() -> &'static str {
    "<path fill=\"#ffffff\" d=\"M0 0H128V128H0Z\"/>"
}

pub fn generate(root: &Path) -> Result<()> {
    fs::create_dir_all(root.join("images"))?;
    fs::create_dir_all(root.join("gt"))?;
    let mut scenes: Vec<(String, String, String, u16, bool, bool)> = Vec::new();
    let mut add = |name: &str, body: &str, colors: u16, aa: bool| {
        scenes.push((
            name.into(),
            "synthetic".into(),
            svg(&format!("{}{body}", background())),
            colors,
            aa,
            false,
        ))
    };
    add(
        "circle_r50",
        "<circle cx=\"64\" cy=\"64\" r=\"50\" fill=\"#102060\"/>",
        2,
        true,
    );
    add(
        "square_rot30",
        "<path transform=\"rotate(30 64 64)\" fill=\"#2060c0\" d=\"M32 32H96V96H32Z\"/>",
        2,
        true,
    );
    let star = (0..10)
        .map(|i| {
            let angle = -std::f64::consts::FRAC_PI_2 + i as f64 * std::f64::consts::PI / 5.0;
            let r = if i % 2 == 0 { 50.0 } else { 22.0 };
            format!("{},{}", 64.0 + r * angle.cos(), 64.0 + r * angle.sin())
        })
        .collect::<Vec<_>>()
        .join(" ");
    add(
        "star5",
        &format!("<polygon fill=\"#c04020\" points=\"{star}\"/>"),
        2,
        true,
    );
    add("ring","<path fill=\"#102060\" fill-rule=\"evenodd\" d=\"M114 64A50 50 0 1 0 14 64A50 50 0 1 0 114 64ZM89 64A25 25 0 1 0 39 64A25 25 0 1 0 89 64Z\"/>",2,true);
    add("concentric","<circle cx=\"64\" cy=\"64\" r=\"50\" fill=\"#102060\"/><circle cx=\"64\" cy=\"64\" r=\"34\" fill=\"#e08020\"/><circle cx=\"64\" cy=\"64\" r=\"16\" fill=\"#20a080\"/>",4,true);
    add("junction_t","<path fill=\"#c03030\" d=\"M0 0H128V64H0Z\"/><path fill=\"#208050\" d=\"M0 64H64V128H0Z\"/><path fill=\"#2040c0\" d=\"M64 64H128V128H64Z\"/>",3,false);
    add("junction_y","<path fill=\"#c03030\" d=\"M0 0H128L64 64Z\"/><path fill=\"#208050\" d=\"M0 0L64 64V128H0Z\"/><path fill=\"#2040c0\" d=\"M128 0V128H64V64Z\"/>",3,true);
    add(
        "line_1px",
        "<path fill=\"#000000\" d=\"M12 63H116V64H12Z\"/>",
        2,
        false,
    );
    add(
        "line_2px",
        "<path fill=\"#000000\" d=\"M63 12H65V116H63Z\"/>",
        2,
        false,
    );
    add(
        "checker",
        "<path fill=\"#000000\" d=\"M16 16H64V64H16ZM64 64H112V112H64Z\"/>",
        2,
        false,
    );
    add(
        "diagonal_touch",
        "<path fill=\"#000000\" d=\"M32 32H64V64H32ZM64 64H96V96H64Z\"/>",
        2,
        false,
    );
    add("nested_squares","<path fill=\"#e08020\" fill-rule=\"evenodd\" d=\"M12 12H116V116H12ZM28 28H100V100H28Z\"/><path fill=\"#2040a0\" d=\"M44 44H84V84H44Z\"/>",3,false);
    add(
        "bezier_leaf",
        "<path fill=\"#208050\" d=\"M16 112C10 30 60 8 112 16C118 78 80 118 16 112Z\"/>",
        2,
        true,
    );
    add(
        "rounded_rectangle",
        "<rect x=\"16\" y=\"24\" width=\"96\" height=\"80\" rx=\"18\" fill=\"#2040a0\"/>",
        2,
        true,
    );
    add(
        "ellipse",
        "<ellipse cx=\"64\" cy=\"64\" rx=\"50\" ry=\"28\" fill=\"#c03030\"/>",
        2,
        true,
    );
    add("text_path","<path fill=\"#101010\" fill-rule=\"evenodd\" d=\"M18 110L52 18H76L110 110H86L78 86H50L42 110ZM58 64H70L64 44Z\"/>",2,true);
    add("triangles","<path fill=\"#c03030\" d=\"M12 112L36 16L60 112Z\"/><path fill=\"#2040a0\" d=\"M68 16L116 64L68 112Z\"/>",3,true);
    add(
        "circle_noaa",
        "<circle cx=\"64\" cy=\"64\" r=\"50\" fill=\"#102060\"/>",
        2,
        false,
    );
    add("multicolor_logo","<path fill=\"#c03030\" d=\"M16 16H64V64H16Z\"/><path fill=\"#208050\" d=\"M64 16H112V64H64Z\"/><path fill=\"#2040c0\" d=\"M16 64H64V112H16Z\"/><circle cx=\"88\" cy=\"88\" r=\"24\" fill=\"#e08020\"/>",5,true);
    add(
        "jpeg_q60",
        "<circle cx=\"64\" cy=\"64\" r=\"40\" fill=\"#102060\"/>",
        2,
        true,
    );
    if let Some(last) = scenes.last_mut() {
        last.5 = true;
    }
    for i in 0..5 {
        let body=format!("{}<path fill=\"#183060\" d=\"M12 112V16H{}V40H40V56H88V80H40V112Z\"/><circle cx=\"96\" cy=\"{}\" r=\"12\" fill=\"#d06820\"/>",background(),64+i*8,24+i*16);
        scenes.push((
            format!("logo_original_{}", i + 1),
            "logo".into(),
            svg(&body),
            3,
            true,
            false,
        ));
    }
    let mut manifest = Vec::new();
    for (id, category, gt, colors, aa, jpeg) in scenes {
        let gtfile = format!("gt/{id}.svg");
        let file = format!("images/{id}.{}", if jpeg { "jpg" } else { "png" });
        fs::write(root.join(&gtfile), &gt)?;
        let raster = av_metrics::render::svg(&gt, aa)?;
        if jpeg {
            let rgb: Vec<_> = raster.px.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
            image::codecs::jpeg::JpegEncoder::new_with_quality(
                fs::File::create(root.join(&file))?,
                60,
            )
            .encode(&rgb, 128, 128, image::ExtendedColorType::Rgb8)?;
        } else {
            save_png(&root.join(&file), &raster)?;
        }
        let geometry = av_metrics::geometry::inspect(&gt)?;
        manifest.push(Entry {
            id,
            file,
            category,
            source: "Generated by av-bench gen; original geometric fixture".into(),
            license: "CC0-1.0".into(),
            expected_colors: Some(colors),
            notes: format!(
                "128x128; AA={aa}; JPEG q60={jpeg}; GT original paths, no external artwork"
            ),
            ground_truth: Some(gtfile),
            corners: geometry.corners,
        });
    }
    for (id, w, h, pixel) in [
        ("edge_1x1", 1, 1, [255, 0, 0, 255]),
        ("edge_transparent", 8, 8, [0, 0, 0, 0]),
        ("edge_uniform", 16, 16, [32, 64, 128, 255]),
        ("edge_1xn", 1, 16, [0, 0, 0, 255]),
    ] {
        let file = format!("images/{id}.png");
        let image = RgbaImage::new(w, h, vec![pixel; (w * h) as usize])?;
        save_png(&root.join(&file), &image)?;
        manifest.push(Entry {
            id: id.into(),
            file,
            category: "edge".into(),
            source: "Generated by av-bench gen".into(),
            license: "CC0-1.0".into(),
            expected_colors: Some(if pixel[3] == 0 { 0 } else { 1 }),
            notes: "Decoder/topology edge case".into(),
            ground_truth: None,
            corners: vec![],
        });
    }
    fs::write(
        root.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;
    println!(
        "generated {} corpus entries (20 synthetic, 5 logo, 4 edge)",
        manifest.len()
    );
    Ok(())
}
