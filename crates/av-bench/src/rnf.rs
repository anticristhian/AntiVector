//! Fresh-process reference workloads: decoded raster through SVG, including detection.
use crate::{runner, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path, process::Command, time::Instant};

#[derive(Serialize, Deserialize)]
struct Trial {
    elapsed_ms: f64,
    peak_working_set_bytes: u64,
    stages_ms: BTreeMap<String, f64>,
    svg_bytes: usize,
    params: av_core::Params,
}

#[cfg(windows)]
fn peak_working_set() -> Result<u64> {
    let result = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!("(Get-Process -Id {}).PeakWorkingSet64", std::process::id()),
        ])
        .output()?;
    if !result.status.success() {
        return Err("PeakWorkingSet query failed".into());
    }
    Ok(String::from_utf8(result.stdout)?.trim().parse()?)
}

#[cfg(not(windows))]
fn peak_working_set() -> Result<u64> {
    Err("RNF high-water mark measurement currently requires Windows".into())
}

pub fn trial(input: &Path, config: &str, colors: u16, output: &Path) -> Result<()> {
    let image = av_session::decode::load(input, 64_000_000)?;
    let mut params = av_core::Params::preset(config)?;
    params.colors = av_core::ColorCount::Fixed(colors);
    params.min_region_area =
        av_core::params::default_min_region_area(params.detail, image.w, image.h);
    let start = Instant::now();
    let (svg, _, stages_ms) = runner::vectorize(&image, &params, false, true)?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    // OS lifetime high-water mark, fresh process for each trial. Query after timing.
    let peak_working_set_bytes = peak_working_set()?;
    fs::write(
        output,
        serde_json::to_vec_pretty(&Trial {
            elapsed_ms,
            peak_working_set_bytes,
            stages_ms,
            svg_bytes: svg.len(),
            params,
        })?,
    )?;
    fs::write(output.with_extension("svg"), svg)?;
    Ok(())
}

pub fn profile(input: &Path, config: &str, colors: u16) -> Result<()> {
    let image = av_session::decode::load(input, 64_000_000)?;
    let mut params = av_core::Params::preset(config)?;
    params.colors = av_core::ColorCount::Fixed(colors);
    params.min_region_area =
        av_core::params::default_min_region_area(params.detail, image.w, image.h);
    for _ in 0..5 {
        let (svg, _, stages) = runner::vectorize(&image, &params, false, true)?;
        println!("profiling only: {} SVG bytes; stages {stages:?}", svg.len());
        std::hint::black_box(svg);
    }
    Ok(())
}

fn fixture(size: u32, colors: u16) -> String {
    let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 1000 1000\"><path fill=\"white\" d=\"M0 0H1000V1000H0Z\"/>");
    let columns = if colors == 8 { 3 } else { 6 };
    let cell = 900 / columns;
    for i in 0..u32::from(colors - 1) {
        let n = (i * 37 + 11) % 64;
        let rgb = [32 + n % 4 * 64, 32 + (n / 4) % 4 * 64, 32 + n / 16 * 64];
        let x = 50 + (i % columns) * cell;
        let y = 50 + (i / columns) * cell;
        let width = cell - 20;
        if i % 3 == 0 {
            svg.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"rgb({},{},{})\"/>",
                x + width / 2,
                y + width / 2,
                width / 2,
                rgb[0],
                rgb[1],
                rgb[2]
            ));
        } else {
            svg.push_str(&format!("<rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{width}\" rx=\"{}\" fill=\"rgb({},{},{})\"/>",if i%3==1 {20}else{0},rgb[0],rgb[1],rgb[2]));
        }
    }
    svg.push_str("</svg>");
    svg
}

pub fn run(root: &Path, label: &str) -> Result<()> {
    runner::safe_name(label)?;
    let directory = root.join(label);
    fs::create_dir_all(&directory)?;
    let mut rows = Vec::new();
    let mut accepted = true;
    for (name, size, colors, config, limit_ms) in [
        ("logo-1mp-8", 1000, 8, "logo", 1000.0),
        ("illustration-4mp-32", 2000, 32, "illustration", 4000.0),
    ] {
        let scene = fixture(size, colors);
        fs::write(directory.join(format!("{name}.svg")), &scene)?;
        let image = av_metrics::render::svg(&scene, true)?;
        let input = directory.join(format!("{name}.png"));
        fs::write(&input, av_session::decode::png_bytes(&image)?)?;
        drop(image);
        let mut trials = Vec::new();
        for index in 0..5 {
            let path = directory.join(format!("{name}-{index}.json"));
            let status = Command::new(std::env::current_exe()?)
                .env("RAYON_NUM_THREADS", "8")
                .arg("rnf-trial")
                .arg(&input)
                .arg(config)
                .arg(colors.to_string())
                .arg(&path)
                .status()?;
            if !status.success() {
                return Err("RNF child failed".into());
            }
            trials.push(serde_json::from_slice::<Trial>(&fs::read(path)?)?);
        }
        let mut times: Vec<_> = trials.iter().map(|t| t.elapsed_ms).collect();
        times.sort_by(f64::total_cmp);
        let peak = trials
            .iter()
            .map(|t| t.peak_working_set_bytes)
            .max()
            .unwrap();
        let memory_limit = u64::from(size) * u64::from(size) * 40 + 200_000_000;
        let passed = times[2] <= limit_ms && peak <= memory_limit;
        accepted &= passed;
        println!(
            "{name}: median={}ms peak={peak}bytes accepted={passed}",
            times[2]
        );
        rows.push(
            serde_json::json!({"name":name,"width":size,"height":size,"workers":8,
            "median_ms":times[2],"limit_ms":limit_ms,"peak_working_set_bytes":peak,
            "memory_limit_bytes":memory_limit,"passed":passed,"trials":trials}),
        );
    }
    fs::write(
        directory.join("metrics.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "profile":"release","scope":"decoded raster through SVG including detection; fresh process per trial",
            "memory_method":"Windows OS PeakWorkingSet64, lifetime high-water mark",
            "fixtures":"original deterministic geometric reference scenes, not a worst-case guarantee",
            "passed":accepted,"rows":rows
        }))?,
    )?;
    if !accepted {
        return Err("RNF reference limits not met; see measured metrics.json".into());
    }
    Ok(())
}
