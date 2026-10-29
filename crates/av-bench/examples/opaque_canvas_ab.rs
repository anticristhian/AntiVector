//! Reproduce the opaque-canvas exporter experiment over the complete corpus.
//! Requires the immutable f9-complete run; original SVGs and human votes are read-only.
use av_core::{Control, Params};
use av_export::svg::SvgOptions;
use serde_json::{json, Value};
use std::{error::Error, fs, path::Path, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        return Err("run the A/B in release mode".into());
    }
    let mut report: Value =
        serde_json::from_slice(&fs::read("bench-out/f9-complete/metrics.json")?)?;
    let manifest: Vec<Value> = serde_json::from_slice(&fs::read("corpus/manifest.json")?)?;
    let output = Path::new("bench-out/f9-opaque-canvas-ab");
    for dir in ["svgs", "renders", "diffs"] {
        fs::create_dir_all(output.join(dir))?;
    }
    let legacy = SvgOptions {
        opaque_canvas: false,
        ..SvgOptions::default()
    };
    let experimental = SvgOptions {
        opaque_canvas: true,
        ..legacy
    };
    let mut rows = Vec::new();
    let mut checks = Vec::new();
    let mut changed = Vec::new();
    for entry in manifest {
        let id = entry["id"].as_str().ok_or("missing image ID")?;
        let path = Path::new("corpus").join(entry["file"].as_str().ok_or("missing source path")?);
        if entry["expected_decode_error"] == true {
            let error = av_session::decode::load(&path, 64_000_000)
                .err()
                .ok_or("corrupt source accepted")?;
            checks.push(
                json!({"id":id,"expected":"decode_error","passed":true,"error":error.to_string()}),
            );
            continue;
        }
        let previous = report["rows"]
            .as_array()
            .ok_or("missing original rows")?
            .iter()
            .find(|row| row["id"] == id)
            .ok_or("missing original row")?;
        let mut params: Params = serde_json::from_value(previous["resolved_params"].clone())?;
        params.experimental.opaque_canvas_overlap = true;
        let source = av_session::decode::load(&path, 64_000_000)?;
        let start = Instant::now();
        let doc = av_core::vectorize(&source, &params, &Control::default())?;
        let before = av_export::svg::to_string_with_options(&doc, &legacy)?;
        let original_svg = fs::read_to_string(format!("bench-out/f9-complete/svgs/{id}.svg"))?;
        if before != original_svg {
            return Err(format!("pre-change pipeline differs: {id}").into());
        }
        let svg = av_export::svg::to_string_with_options(&doc, &experimental)?;
        let total_ms = start.elapsed().as_secs_f64() * 1000.0;
        // Geometry/color coverage must remain byte-exact, not merely have no gaps.
        let coverage = av_metrics::render::svg(&svg, false)?;
        let legacy_coverage = av_metrics::render::svg(&before, false)?;
        if coverage != legacy_coverage {
            return Err(format!("non-AA geometry/color changed: {id}").into());
        }
        let render = av_metrics::render::svg(&svg, true)?;
        let fidelity =
            av_metrics::compare(&source, &render, &coverage, params.transparency_alpha_cut)?;
        let geometry = av_metrics::geometry::inspect(&svg)?;
        let mut row = previous.clone();
        row["resolved_params"] = serde_json::to_value(&params)?;
        for (key, value) in serde_json::to_value(&fidelity)?
            .as_object()
            .ok_or("invalid fidelity")?
        {
            row[key] = value.clone();
        }
        row["paths"] = json!(geometry.paths);
        row["nodes"] = json!(geometry.nodes);
        row["svg_bytes"] = json!(svg.len());
        row["total_ms"] = json!(total_ms);
        row["stages_ms"] = json!({});
        row["sampled_peak_rss"] = Value::Null;
        if let Some(gt) = entry["ground_truth"].as_str() {
            let truth =
                av_metrics::geometry::inspect(&fs::read_to_string(Path::new("corpus").join(gt))?)?;
            row["hausdorff"] = json!(av_metrics::geometry::hausdorff(&geometry, &truth));
            let corners: Vec<[f64; 2]> = serde_json::from_value(entry["corners"].clone())?;
            row["corner_recall"] = json!(av_metrics::geometry::corner_recall(&corners, &geometry));
        }
        let different = svg != before;
        if id == "photo_chelsea" {
            let revision = av_session::editor::Revision {
                params: params.clone(),
                edits: Vec::new(),
                doc: std::sync::Arc::new(doc.clone()),
            };
            av_session::project::save(
                &output.join("photo_chelsea-optin.avproj"),
                "photo_chelsea.png",
                &source,
                &revision,
            )
            .map_err(|error| error.to_string())?;
        }
        if different {
            changed.push(id.to_string());
        }
        println!(
            "{id}: changed={different}; SSIM {:.9}->{:.9}; DE {:.9}->{:.9}; nodes {}->{}",
            previous["ssim"].as_f64().ok_or("missing SSIM")?,
            fidelity.ssim,
            previous["delta_e_mean"].as_f64().ok_or("missing DE")?,
            fidelity.delta_e_mean,
            previous["nodes"],
            geometry.nodes
        );
        fs::write(output.join("svgs").join(format!("{id}.svg")), svg)?;
        fs::write(
            output.join("renders").join(format!("{id}.png")),
            av_session::decode::png_bytes(&render)?,
        )?;
        rows.push(row);
    }
    report["label"] = json!("f9-opaque-canvas-ab");
    report["params"]["experimental"]["opaque_canvas_overlap"] = json!(true);
    report["engine"] = json!("AntiVector opaque-canvas exporter experiment");
    report["rows"] = json!(rows);
    report["input_checks"] = json!(checks);
    report["memory_method"] = json!("not measured in exporter A/B");
    report["experiment"] = json!({"changed":changed,"non_aa_render_identical":true,
        "legacy_svg_identical_to_evaluated_candidate":true,
        "time_scope":"pipeline plus both SVG serializations, not RNF"});
    fs::write(
        output.join("metrics.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
