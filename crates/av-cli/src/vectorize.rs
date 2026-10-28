//! Vectorization CLI with quality presets, composition options and stage images.

use av_core::{
    color::{Palette, TRANSPARENT},
    pipeline::{finish, polygon_stages},
    ColorCount, Control, CoreError, Params,
};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct VectorizeArgs {
    #[arg(long,value_parser=["exact","overlap"])]
    pub stacking: Option<String>,
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: PathBuf,
    #[arg(long, default_value = "logo")]
    pub preset: String,
    #[arg(long)]
    pub params: Option<PathBuf>,
    #[arg(long)]
    pub colors: Option<u16>,
    #[arg(long,num_args=0..=1,default_missing_value="bench-out/stages")]
    pub dump_stages: Option<PathBuf>,
}

type Failure = (u8, String);

fn core_error(error: CoreError) -> Failure {
    (
        if matches!(error, CoreError::Internal(_)) {
            10
        } else {
            2
        },
        error.to_string(),
    )
}

fn raster(
    w: u32,
    h: u32,
    labels: impl Iterator<Item = u16>,
    palette: &Palette,
) -> image::RgbaImage {
    let mut image = image::RgbaImage::new(w, h);
    for (pixel, label) in image.pixels_mut().zip(labels) {
        if label == TRANSPARENT {
            pixel.0 = [0; 4];
        } else {
            let rgb = palette.colors[label as usize].rgb;
            pixel.0 = [rgb[0], rgb[1], rgb[2], 255];
        }
    }
    image
}

pub fn run(args: VectorizeArgs) -> Result<(), Failure> {
    let mut params = if let Some(path) = &args.params {
        Params::from_json(&std::fs::read_to_string(path).map_err(|e| (4, e.to_string()))?)
            .map_err(core_error)?
    } else {
        Params::preset(&args.preset).map_err(core_error)?
    };
    if let Some(n) = args.colors {
        params.colors = ColorCount::Fixed(n);
    }
    params.validate().map_err(core_error)?;
    let image = crate::decode::load(&args.input, av_core::raster::DEFAULT_PIXEL_LIMIT)
        .map_err(|e| (e.exit_code(), e.to_string()))?;
    if args.params.is_none() {
        params.min_region_area =
            av_core::params::default_min_region_area(params.detail, image.w, image.h);
        if params.experimental.quality_v5 {
            params.denoise =
                av_core::params::resolve_defaults(params.mode, &av_core::detect(&image)).denoise;
            if params.experimental.force_pixel_art {
                params.denoise = 0.0;
                params.min_region_area = 1;
            }
        }
    }
    let ctl = Control::default();
    let stages = polygon_stages(&image, &params, &ctl).map_err(core_error)?;
    if let Some(path) = args.dump_stages {
        std::fs::create_dir_all(&path).map_err(|e| (4, e.to_string()))?;
        raster(
            stages.quantized.w,
            stages.quantized.h,
            stages.quantized.labels.iter().copied(),
            &stages.quant_palette,
        )
        .save(path.join("quantize.png"))
        .map_err(|e| (4, e.to_string()))?;
        raster(
            stages.resolved.w,
            stages.resolved.h,
            stages.resolved.labels.iter().copied(),
            &stages.resolved_palette,
        )
        .save(path.join("resolve_aa.png"))
        .map_err(|e| (4, e.to_string()))?;
        let bytes: Vec<u8> = stages.preprocessed.px.iter().flatten().copied().collect();
        image::save_buffer(
            path.join("preprocess.png"),
            &bytes,
            stages.preprocessed.w,
            stages.preprocessed.h,
            image::ColorType::Rgba8,
        )
        .map_err(|e| (4, e.to_string()))?;
        for (name, map, palette) in [
            ("label", &stages.labeled, &stages.resolved_palette),
            ("merge_small", &stages.merged, &stages.palette),
        ] {
            raster(
                map.w,
                map.h,
                map.region_of
                    .iter()
                    .map(|&id| map.regions[id as usize].color),
                palette,
            )
            .save(path.join(format!("{name}.png")))
            .map_err(|e| (4, e.to_string()))?;
        }
    }
    let doc = finish(&stages, &ctl).map_err(core_error)?;
    let options = av_export::svg::SvgOptions {
        overlap: args.stacking.as_deref().map_or_else(
            || av_export::svg::SvgOptions::default().overlap,
            |mode| mode == "overlap",
        ),
    };
    let svg = av_export::svg::to_string_with_options(&doc, &options).map_err(core_error)?;
    std::fs::write(&args.output, svg).map_err(|e| (4, e.to_string()))?;
    println!(
        "paths={} nodes={} colors={}",
        doc.stats.paths, doc.stats.nodes, doc.stats.colors
    );
    Ok(())
}
