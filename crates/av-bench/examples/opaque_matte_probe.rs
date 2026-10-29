//! Diagnostic experiment after the first human F9 assessment; never changes defaults.
//! Measures an existing-palette matte only for fully opaque inputs. Coverage is
//! checked on the original candidate, so the experiment cannot hide geometry gaps.
use std::{collections::BTreeMap, error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let output = Path::new("bench-out/f9-opaque-matte-probe");
    fs::create_dir_all(output)?;
    let mut rows = Vec::new();
    for id in [
        "photo_camera",
        "photo_coffee",
        "photo_chelsea",
        "photo_brick",
        "photo_astronaut",
        "illustration_sailboat",
    ] {
        let source = av_session::decode::load(
            &Path::new("corpus/images").join(format!("{id}.png")),
            64_000_000,
        )?;
        if source.px.iter().any(|pixel| pixel[3] != 255) {
            return Err("probe only supports fully opaque source inputs".into());
        }
        let svg = fs::read_to_string(format!("bench-out/f9-complete/svgs/{id}.svg"))?;
        let before = av_metrics::render::svg(&svg, true)?;
        let coverage = av_metrics::render::svg(&svg, false)?;
        let mut palette = BTreeMap::<[u8; 3], usize>::new();
        for text in svg.split("fill=\"#").skip(1) {
            let color = std::array::from_fn(|i| {
                u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).expect("writer hex RGB")
            });
            palette.entry(color).or_default();
        }
        for pixel in &before.px {
            if pixel[3] == 255 {
                if let Some(count) = palette.get_mut(&[pixel[0], pixel[1], pixel[2]]) {
                    *count += 1;
                }
            }
        }
        let (color, _) = palette
            .iter()
            .max_by_key(|(_, count)| **count)
            .ok_or("empty palette")?;
        let (header, paths) = svg.split_once('>').ok_or("missing SVG root")?;
        let modified = format!(
            "{header}><path fill=\"#{:02x}{:02x}{:02x}\" d=\"M0 0H{}V{}H0Z\"/>{paths}",
            color[0], color[1], color[2], source.w, source.h
        );
        let after = av_metrics::render::svg(&modified, true)?;
        let baseline = av_metrics::compare(&source, &before, &coverage, 128)?;
        let experimental = av_metrics::compare(&source, &after, &coverage, 128)?;
        let partial =
            |image: &av_core::RgbaImage| image.px.iter().filter(|pixel| pixel[3] < 255).count();
        println!(
            "{id}: SSIM {:.9} -> {:.9}; DE {:.9} -> {:.9}",
            baseline.ssim, experimental.ssim, baseline.delta_e_mean, experimental.delta_e_mean
        );
        rows.push(
            serde_json::json!({"id":id,"matte_rgb":color,"before":baseline,
            "experimental":experimental,"nonopaque_before":partial(&before),
            "nonopaque_after":partial(&after),"additional_anchors":4,
            "coverage_source":"unmodified candidate, not experimental matte"}),
        );
        fs::write(output.join(format!("{id}.svg")), modified)?;
        fs::write(
            output.join(format!("{id}.png")),
            av_session::decode::png_bytes(&after)?,
        )?;
    }
    fs::write(
        output.join("metrics.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    Ok(())
}
