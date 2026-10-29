//! Bounded fitting-parameter sweep after the human assessment. No preset changes.
//! All 65 non-edge inputs are measured, including controls that won or tied.
use av_core::{Control, Params};
use av_export::svg::SvgOptions;
use serde_json::{json, Value};
use std::{error::Error, fs, path::Path, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        return Err("use release mode".into());
    }
    let base: Value =
        serde_json::from_slice(&fs::read("bench-out/f9-opaque-canvas-ab/metrics.json")?)?;
    let manifest: Vec<Value> = serde_json::from_slice(&fs::read("corpus/manifest.json")?)?;
    let options = SvgOptions {
        opaque_canvas: true,
        ..SvgOptions::default()
    };
    for (name, smoothness, tolerance_scale) in [
        ("f9-fit-tight", None, 0.5),
        ("f9-fit-smooth", Some(1.0), 1.0),
        ("f9-fit-smooth-tight", Some(1.0), 0.5),
    ] {
        let output = Path::new("bench-out").join(name);
        for dir in ["svgs", "renders", "diffs"] {
            fs::create_dir_all(output.join(dir))?;
        }
        let mut rows = Vec::new();
        for entry in &manifest {
            if entry["category"] == "edge" {
                continue;
            }
            let id = entry["id"].as_str().ok_or("missing ID")?;
            let previous = base["rows"]
                .as_array()
                .ok_or("missing rows")?
                .iter()
                .find(|row| row["id"] == id)
                .ok_or("missing base row")?;
            let mut params: Params = serde_json::from_value(previous["resolved_params"].clone())?;
            params.experimental.opaque_canvas_overlap = true;
            if let Some(value) = smoothness {
                params.smoothness = value;
            }
            params.fit_tolerance_px *= tolerance_scale;
            let source = av_session::decode::load(
                &Path::new("corpus").join(entry["file"].as_str().ok_or("source path")?),
                64_000_000,
            )?;
            let start = Instant::now();
            let doc = av_core::vectorize(&source, &params, &Control::default())?;
            let svg = av_export::svg::to_string_with_options(&doc, &options)?;
            let total_ms = start.elapsed().as_secs_f64() * 1000.0;
            let render = av_metrics::render::svg(&svg, true)?;
            let coverage = av_metrics::render::svg(&svg, false)?;
            let fidelity =
                av_metrics::compare(&source, &render, &coverage, params.transparency_alpha_cut)?;
            let geometry = av_metrics::geometry::inspect(&svg)?;
            let mut row = previous.clone();
            for (key, value) in serde_json::to_value(&fidelity)?
                .as_object()
                .ok_or("fidelity")?
            {
                row[key] = value.clone();
            }
            row["resolved_params"] = serde_json::to_value(&params)?;
            row["paths"] = json!(geometry.paths);
            row["nodes"] = json!(geometry.nodes);
            row["svg_bytes"] = json!(svg.len());
            row["total_ms"] = json!(total_ms);
            row["stages_ms"] = json!({});
            row["sampled_peak_rss"] = Value::Null;
            if let Some(gt) = entry["ground_truth"].as_str() {
                let truth = av_metrics::geometry::inspect(&fs::read_to_string(
                    Path::new("corpus").join(gt),
                )?)?;
                row["hausdorff"] = json!(av_metrics::geometry::hausdorff(&geometry, &truth));
                let corners: Vec<[f64; 2]> = serde_json::from_value(entry["corners"].clone())?;
                row["corner_recall"] =
                    json!(av_metrics::geometry::corner_recall(&corners, &geometry));
            }
            let diff = av_core::RgbaImage::new(
                source.w,
                source.h,
                source
                    .px
                    .iter()
                    .zip(&render.px)
                    .map(|(&a, &b)| {
                        let a = av_metrics::composite_white(a);
                        let b = av_metrics::composite_white(b);
                        [
                            a[0].abs_diff(b[0]),
                            a[1].abs_diff(b[1]),
                            a[2].abs_diff(b[2]),
                            255,
                        ]
                    })
                    .collect(),
            )?;
            fs::write(output.join("svgs").join(format!("{id}.svg")), svg)?;
            fs::write(
                output.join("renders").join(format!("{id}.png")),
                av_session::decode::png_bytes(&render)?,
            )?;
            fs::write(
                output.join("diffs").join(format!("{id}.png")),
                av_session::decode::png_bytes(&diff)?,
            )?;
            println!(
                "{name}/{id}: SSIM {:.9}->{:.9}; DE {:.9}->{:.9}; nodes {}->{}",
                previous["ssim"].as_f64().ok_or("SSIM")?,
                fidelity.ssim,
                previous["delta_e_mean"].as_f64().ok_or("DE")?,
                fidelity.delta_e_mean,
                previous["nodes"],
                geometry.nodes
            );
            rows.push(row);
        }
        let mut report = base.clone();
        report["label"] = json!(name);
        report["engine"] = json!("AntiVector fitting parameter experiment");
        report["rows"] = json!(rows);
        report["input_checks"] = json!([]);
        report["experiment"] = json!({"scope":"all 65 non-edge inputs; not full corpus/RNF", "smoothness":smoothness,"tolerance_scale":tolerance_scale});
        fs::write(
            output.join("metrics.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;
    }
    Ok(())
}
