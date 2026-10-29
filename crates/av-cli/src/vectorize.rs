//! Vectorization CLI with quality presets, composition options and stage images.

use av_core::{
    color::{Palette, TRANSPARENT},
    pipeline::{finish, polygon_stages},
    ColorCount, Control, CoreError, Params,
};
use std::path::PathBuf;

#[derive(Clone, clap::Args)]
pub struct VectorizeArgs {
    #[command(flatten)]
    pub settings: Settings,
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: PathBuf,
    #[arg(long,num_args=0..=1,default_missing_value="bench-out/stages")]
    pub dump_stages: Option<PathBuf>,
}

#[derive(Clone, clap::Args)]
pub struct Settings {
    #[arg(long,value_parser=["exact","overlap"])]
    pub stacking: Option<String>,
    /// Override the output extension (svg/pdf/eps/dxf/png).
    #[arg(long,value_parser=["svg","pdf","eps","dxf","png"])]
    pub format: Option<String>,
    #[arg(long,default_value_t=2,value_parser=clap::value_parser!(u8).range(0..=8))]
    pub precision: u8,
    /// Combine equal RGB colors into one SVG path; requires exact stacking.
    #[arg(long)]
    pub group_by_color: bool,
    #[arg(long,default_value_t=av_core::raster::DEFAULT_PIXEL_LIMIT)]
    pub max_pixels: u64,
    #[arg(long, default_value = "logo")]
    pub preset: String,
    #[arg(long)]
    pub params: Option<PathBuf>,
    #[arg(long)]
    pub colors: Option<u16>,
}

pub type Failure = (u8, String);

impl Settings {
    pub fn options(&self) -> Result<av_export::svg::SvgOptions, Failure> {
        let options = av_export::svg::SvgOptions {
            overlap: self
                .stacking
                .as_deref()
                .map_or(!self.group_by_color, |s| s == "overlap"),
            precision: self.precision,
            group_by_color: self.group_by_color,
        };
        options.validate().map_err(core_error)?;
        Ok(options)
    }
    pub fn parameters(&self) -> Result<Params, Failure> {
        let mut p = if let Some(path) = &self.params {
            Params::from_json(&std::fs::read_to_string(path).map_err(|e| (4, e.to_string()))?)
                .map_err(core_error)?
        } else {
            Params::preset(&self.preset).map_err(core_error)?
        };
        if let Some(n) = self.colors {
            p.colors = ColorCount::Fixed(n);
        }
        p.validate().map_err(core_error)?;
        Ok(p)
    }
}

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
    let options = args.settings.options()?;
    let format = av_export::Format::parse(
        args.settings
            .format
            .as_deref()
            .or_else(|| args.output.extension().and_then(|e| e.to_str()))
            .ok_or_else(|| (2, "output needs a supported extension or --format".into()))?,
    )
    .map_err(core_error)?;
    let mut params = args.settings.parameters()?;
    if let (Ok(input), Ok(output)) = (args.input.canonicalize(), args.output.canonicalize()) {
        if input == output {
            return Err((2, "output must differ from input".into()));
        }
    }
    let image = crate::decode::load(&args.input, args.settings.max_pixels)
        .map_err(|e| (e.exit_code(), e.to_string()))?;
    if args.settings.params.is_none() {
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
    let bytes = av_export::export(&doc, format, &options).map_err(core_error)?;
    let stats = av_export::statistics(&doc, format, &options).map_err(core_error)?;
    std::fs::write(&args.output, bytes).map_err(|e| (4, e.to_string()))?;
    println!(
        "paths={} nodes={} colors={}",
        stats.paths, stats.nodes, stats.colors
    );
    Ok(())
}
