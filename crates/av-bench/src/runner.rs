//! Stage timing, sampled process memory, rendering, baseline and fidelity measurements.

use crate::{
    corpus::{self, Entry},
    Result,
};
use av_core::{
    assemble::{assemble, polygon_edges},
    stages, Control, Params, RgbaImage,
};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    #[serde(default)]
    pub resolved_params: Option<Params>,
    pub id: String,
    pub category: String,
    pub paths: usize,
    pub nodes: usize,
    pub colors: usize,
    pub input_palette_colors: usize,
    pub svg_bytes: usize,
    #[serde(flatten)]
    pub fidelity: av_metrics::Fidelity,
    pub hausdorff: Option<f64>,
    pub corner_recall: Option<f64>,
    pub total_ms: f64,
    pub stages_ms: BTreeMap<String, f64>,
    pub sampled_peak_rss: Option<usize>,
    pub detection: av_core::detect::Detection,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunReport {
    #[serde(default)]
    pub overlap: bool,
    pub label: String,
    pub engine: String,
    pub params: Params,
    pub profile: String,
    pub os: String,
    pub memory_method: String,
    pub rows: Vec<Row>,
}

pub fn load_report(path: &Path) -> Result<RunReport> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn params(config: &str, quantizer: Option<&str>) -> Result<Params> {
    let mut params = if Path::new(config).is_file() {
        Params::from_json(&fs::read_to_string(config)?)?
    } else {
        Params::preset(config)?
    };
    if let Some(name) = quantizer {
        params.experimental.quantizer = match name {
            "kmeans" => av_core::params::Quantizer::Kmeans,
            "median_cut" => av_core::params::Quantizer::MedianCut,
            "wu" => av_core::params::Quantizer::Wu,
            _ => return Err("quantizer must be kmeans, median_cut or wu".into()),
        };
    }
    Ok(params)
}

pub fn safe_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err("label/id must contain only ASCII letters, digits, _, - or .".into());
    }
    Ok(())
}

fn timed<T>(
    times: &mut BTreeMap<String, f64>,
    name: &str,
    f: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let start = Instant::now();
    let output = f()?;
    times.insert(name.into(), start.elapsed().as_secs_f64() * 1000.0);
    Ok(output)
}

fn vectorize(
    image: &RgbaImage,
    p: &Params,
    baseline: bool,
    overlap: bool,
) -> Result<(String, usize, BTreeMap<String, f64>)> {
    let ctl = Control::default();
    let mut times = BTreeMap::new();
    let original_size = (image.w, image.h);
    let mut detection = timed(&mut times, "detect", || Ok(av_core::detect(image)))?;
    detection.is_pixel_art |= p.experimental.force_pixel_art;
    let (prepared, scale) = if p.experimental.quality_v5 {
        let prepared = timed(&mut times, "preprocess", || {
            Ok(stages::preprocess(image, p, &detection, &ctl)?)
        })?;
        (Cow::Owned(prepared.image), prepared.scale)
    } else {
        (Cow::Borrowed(image), 1)
    };
    let image = prepared.as_ref();
    let scaled = av_core::pipeline::scaled_params(p, scale);
    let p = &scaled;
    let (palette, labels) = timed(&mut times, "quantize", || {
        Ok(stages::quantize(image, p, &ctl)?)
    })?;
    let (palette, labels, mixture) = if p.experimental.quality_v5 {
        let aa = timed(&mut times, "resolve_aa", || {
            Ok(stages::resolve_aa(image, &palette, &labels, p, &ctl)?)
        })?;
        (aa.palette, aa.labels, Some(aa.mixture_pixels))
    } else {
        (palette, labels, None)
    };
    // F5 compares identical final segmentation, including removed AA/noise colors.
    // Historical F3/F4 baselines retain their original quantization-only input.
    let mut palette = palette;
    let common = if p.experimental.quality_v5 {
        let labeled = timed(&mut times, "label", || {
            Ok(stages::label(&labels, Some(image), &ctl)?)
        })?;
        let mut merged = timed(&mut times, "merge_small", || {
            Ok(stages::merge_small_with_aa(
                &labeled,
                &palette,
                p,
                Some(image),
                mixture.as_ref().unwrap(),
                &ctl,
            )?)
        })?;
        palette = av_core::color::compact_regions(&palette, &mut merged)?;
        Some(merged)
    } else {
        None
    };
    let count = palette.colors.len();
    if baseline {
        let common_labels = common.as_ref().map(|map| {
            map.region_of
                .iter()
                .map(|&id| map.regions[id as usize].color)
                .collect::<Vec<_>>()
        });
        let labels = common_labels.as_deref().unwrap_or(&labels.labels);
        if labels.iter().all(|&id| id == av_core::color::TRANSPARENT) {
            return Ok((
                format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\"/>",
                    original_size.0, original_size.1
                ),
                count,
                times,
            ));
        }
        // Same palette labels; do not reduce precision or remove speckles afterwards.
        let pixels = labels
            .iter()
            .flat_map(|&id| {
                if id == av_core::color::TRANSPARENT {
                    [0, 0, 0, 0]
                } else {
                    let rgb = palette.colors[id as usize].rgb;
                    [rgb[0], rgb[1], rgb[2], 255]
                }
            })
            .collect();
        let svg = timed(&mut times, "vtracer", || {
            let config = vtracer::Config {
                color_precision: 8,
                layer_difference: 0,
                filter_speckle: 0,
                hierarchical: if overlap {
                    vtracer::Hierarchical::Stacked
                } else {
                    vtracer::Hierarchical::Cutout
                },
                ..Default::default()
            };
            Ok(vtracer::convert(
                vtracer::ColorImage {
                    pixels,
                    width: image.w as usize,
                    height: image.h as usize,
                },
                config,
            )
            .map_err(std::io::Error::other)?
            .to_string())
        })?;
        let svg = if scale > 1 {
            let needle = format!("width=\"{}\" height=\"{}\"", image.w, image.h);
            if !svg.contains(&needle) {
                return Err("vtracer SVG dimensions missing".into());
            }
            svg.replacen(
                &needle,
                &format!(
                    "width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"",
                    original_size.0, original_size.1, image.w, image.h
                ),
                1,
            )
        } else {
            svg
        };
        return Ok((svg, count, times));
    }
    let merged = if let Some(merged) = common {
        merged
    } else {
        let labeled = timed(&mut times, "label", || {
            Ok(stages::label(&labels, Some(image), &ctl)?)
        })?;
        timed(&mut times, "merge_small", || {
            if let Some(mask) = &mixture {
                return Ok(stages::merge_small_with_aa(
                    &labeled,
                    &palette,
                    p,
                    Some(image),
                    mask,
                    &ctl,
                )?);
            }
            Ok(stages::merge_small(
                &labeled,
                &palette,
                p,
                Some(image),
                &ctl,
            )?)
        })?
    };
    let graph = timed(&mut times, "extract_boundaries", || {
        Ok(stages::extract_boundaries(&merged, &ctl)?)
    })?;
    let edges = if p.experimental.curve_fit {
        timed(&mut times, "fit_edges", || {
            Ok(av_core::fit::fit_edges_for_regions_at_scale(
                &graph,
                &merged,
                &palette,
                p,
                detection.is_pixel_art,
                scale,
                &ctl,
            )?)
        })?
    } else {
        timed(&mut times, "fit_edges_polygon", || {
            Ok(polygon_edges(&graph))
        })?
    };
    let doc = timed(&mut times, "assemble", || {
        Ok(av_core::pipeline::restore_scale(
            assemble(&merged, &graph, &palette, edges, &ctl)?,
            scale,
        ))
    })?;
    let svg = timed(&mut times, "export_svg", || {
        Ok(av_export::svg::to_string_with_options(
            &doc,
            &av_export::svg::SvgOptions { overlap },
        )?)
    })?;
    Ok((svg, count, times))
}

fn sampled<T>(f: impl FnOnce() -> Result<T>) -> Result<(T, Option<usize>)> {
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let sampler = std::thread::spawn(move || {
        let mut peak = None;
        loop {
            if let Some(stats) = memory_stats::memory_stats() {
                peak = Some(peak.map_or(stats.physical_mem, |v: usize| v.max(stats.physical_mem)));
            }
            if signal.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        peak
    });
    let result = f();
    stop.store(true, Ordering::Relaxed);
    let peak = sampler.join().map_err(|_| "memory sampler panicked")?;
    Ok((result?, peak))
}

fn resolved(base: &Params, image: &RgbaImage, derive_defaults: bool) -> Params {
    let mut p = base.clone();
    if derive_defaults {
        p.min_region_area = av_core::params::default_min_region_area(p.detail, image.w, image.h);
        if p.experimental.quality_v5 {
            p.denoise = av_core::params::resolve_defaults(p.mode, &av_core::detect(image)).denoise;
            if p.experimental.force_pixel_art {
                p.denoise = 0.0;
                p.min_region_area = 1;
            }
        }
    }
    p
}

fn measure(
    root: &Path,
    entry: &Entry,
    p: &Params,
    baseline: bool,
    out: &Path,
    derive_defaults: bool,
    overlap: bool,
) -> Result<Row> {
    safe_name(&entry.id)?;
    let image = corpus::read(root, entry)?;
    let p = resolved(p, &image, derive_defaults);
    let (((svg, palette_count, stages_ms), total_ms), peak) = sampled(|| {
        let start = Instant::now();
        let result = vectorize(&image, &p, baseline, overlap)?;
        Ok((result, start.elapsed().as_secs_f64() * 1000.0))
    })?;
    let render = av_metrics::render::svg(&svg, true)?;
    let coverage = av_metrics::render::svg(&svg, false)?;
    let fidelity = av_metrics::compare(&image, &render, &coverage, p.transparency_alpha_cut)?;
    let geometry = av_metrics::geometry::inspect(&svg)?;
    let (hausdorff, corner_recall) = if let Some(gt) = &entry.ground_truth {
        let truth = av_metrics::geometry::inspect(&fs::read_to_string(root.join(gt))?)?;
        (
            av_metrics::geometry::hausdorff(&geometry, &truth),
            Some(av_metrics::geometry::corner_recall(
                &entry.corners,
                &geometry,
            )),
        )
    } else {
        (None, None)
    };
    let colors: BTreeSet<_> = coverage
        .px
        .iter()
        .filter(|p| p[3] > 0)
        .map(|p| [p[0], p[1], p[2]])
        .collect();
    fs::write(out.join("svgs").join(format!("{}.svg", entry.id)), &svg)?;
    corpus::save_png(
        &out.join("renders").join(format!("{}.png", entry.id)),
        &render,
    )?;
    let diff = RgbaImage::new(
        image.w,
        image.h,
        image
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
    corpus::save_png(&out.join("diffs").join(format!("{}.png", entry.id)), &diff)?;
    Ok(Row {
        resolved_params: Some(p),
        id: entry.id.clone(),
        category: entry.category.clone(),
        paths: geometry.paths,
        nodes: geometry.nodes,
        colors: colors.len(),
        input_palette_colors: palette_count,
        svg_bytes: svg.len(),
        fidelity,
        hausdorff,
        corner_recall,
        total_ms,
        stages_ms,
        sampled_peak_rss: peak,
        detection: av_core::detect(&image),
    })
}

pub fn run(
    root: &Path,
    output_root: &Path,
    label: &str,
    config: &str,
    quantizer: Option<&str>,
    baseline: bool,
    overlap: bool,
) -> Result<RunReport> {
    safe_name(label)?;
    let params = params(config, quantizer)?;
    let out = output_root.join(label);
    for dir in ["svgs", "renders", "diffs"] {
        fs::create_dir_all(out.join(dir))?;
    }
    let mut rows = Vec::new();
    for entry in corpus::load(root)? {
        let row = measure(
            root,
            &entry,
            &params,
            baseline,
            &out,
            !Path::new(config).is_file(),
            overlap,
        )?;
        println!(
            "{} SSIM={:.6} DE00={:.6} gaps={} nodes={} ms={:.3}",
            row.id,
            row.fidelity.ssim,
            row.fidelity.delta_e_mean,
            row.fidelity.gap_pixels,
            row.nodes,
            row.total_ms
        );
        rows.push(row);
    }
    let report = RunReport {
        overlap,
        label: label.into(),
        engine: if baseline {
            "vtracer 0.6.5 (common quantized palette)"
        } else if params.experimental.curve_fit {
            "AntiVector curves (experimental)"
        } else {
            "AntiVector polygon"
        }
        .into(),
        params,
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
        .into(),
        os: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        memory_method:
            "process RSS sampled every 2ms during pipeline; approximate, includes process overhead"
                .into(),
        rows,
    };
    fs::write(
        out.join("metrics.json"),
        serde_json::to_string_pretty(&report)? + "\n",
    )?;
    Ok(report)
}

pub fn perf(
    root: &Path,
    output_root: &Path,
    label: &str,
    config: &str,
    quantizer: Option<&str>,
    overlap: bool,
) -> Result<()> {
    safe_name(label)?;
    let base = params(config, quantizer)?;
    let mut rows = Vec::new();
    for entry in corpus::load(root)? {
        let image = corpus::read(root, &entry)?;
        let p = resolved(&base, &image, !Path::new(config).is_file());
        let mut trials = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            let (_, _, stages) = vectorize(&image, &p, false, overlap)?;
            trials.push((start.elapsed().as_secs_f64() * 1000.0, stages));
        }
        let median = |mut values: Vec<f64>| {
            values.sort_by(f64::total_cmp);
            values[2]
        };
        let mut stage_medians = BTreeMap::new();
        for key in trials[0].1.keys() {
            stage_medians.insert(
                key.clone(),
                median(trials.iter().map(|t| t.1[key]).collect()),
            );
        }
        rows.push(serde_json::json!({"id":entry.id,"repetitions":5,"median_ms":median(trials.iter().map(|t|t.0).collect()),"median_stages_ms":stage_medians}));
    }
    fs::create_dir_all(output_root.join(label))?;
    fs::write(
        output_root.join(label).join("perf.json"),
        serde_json::to_string_pretty(
            &serde_json::json!({"profile":if cfg!(debug_assertions){"debug"}else{"release"},"rows":rows}),
        )? + "\n",
    )?;
    println!("saved medians of 5 repetitions for {} images", rows.len());
    Ok(())
}
