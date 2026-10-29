//! Reproducible quality harness; vtracer is confined to this binary.

mod corpus;
mod full_corpus;
mod report;
mod rnf;
mod runner;
mod visual_score;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
struct Cli {
    #[arg(long, global = true, default_value = "corpus")]
    corpus: PathBuf,
    #[arg(long, global = true, default_value = "bench-out")]
    output_root: PathBuf,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Score only a completed human evaluation; never generate votes.
    ScoreVisual {
        label: String,
        answers: PathBuf,
        #[arg(long)]
        candidate: String,
    },
    /// Portable CPU profiling workload; instrumented times are not RNF evidence.
    Profile {
        input: PathBuf,
        #[arg(long, default_value = "illustration")]
        config: String,
        #[arg(long, default_value_t = 32)]
        colors: u16,
    },
    /// Measure RNF01–03 using five fresh processes per reference image.
    Rnf {
        #[arg(long, default_value = "rnf")]
        label: String,
    },
    #[command(hide = true)]
    RnfTrial {
        input: PathBuf,
        config: String,
        colors: u16,
        output: PathBuf,
    },
    /// Generate an original 4MP logo fixture for desktop responsiveness checks.
    Fixture4mp { path: PathBuf },
    /// Five real session jobs, measuring progressive preview and full completion.
    SessionPerf {
        input: PathBuf,
        #[arg(long, default_value_t = 250000)]
        preview_pixels: usize,
        #[arg(long)]
        label: String,
    },
    /// Measure independently rasterized exports against source-resolution PNGs.
    CompareRenders {
        reference: PathBuf,
        rendered: PathBuf,
        #[arg(long)]
        label: String,
    },
    /// Check F5 fidelity and equivalent-color compactness on the current corpus.
    GateQuality {
        candidate: PathBuf,
        reference: PathBuf,
    },
    /// Generate anonymous randomized pairs; only a human may fill the answers.
    Visual {
        a: String,
        b: String,
        #[arg(long)]
        label: String,
        #[arg(long, default_value_t = 42)]
        seed: u64,
    },
    /// Check the measured geometric/compactness acceptance criteria of F4.
    GateCurves {
        candidate: PathBuf,
        reference: PathBuf,
    },
    /// Generate original fixtures with SVG ground truth and license manifest.
    Gen {
        #[arg(long)]
        full: bool,
    },
    /// Measure AntiVector using a preset or complete parameter JSON file.
    Run {
        #[arg(long)]
        stacking: Option<String>,
        #[arg(long, default_value = "logo")]
        config: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        quantizer: Option<String>,
        #[arg(long)]
        check_baseline: Option<PathBuf>,
    },
    /// Measure an equivalent-palette vtracer baseline.
    Baseline {
        #[arg(long)]
        stacking: Option<String>,
        engine: String,
        #[arg(long, default_value = "logo")]
        config: String,
        #[arg(long, default_value = "vtracer")]
        label: String,
    },
    /// Generate an HTML comparison with copied previews and difference maps.
    Compare {
        a: String,
        b: String,
        #[arg(long)]
        check: bool,
    },
    /// Five repetitions per corpus image, reporting medians by stage.
    Perf {
        #[arg(long)]
        stacking: Option<String>,
        #[arg(long, default_value = "logo")]
        config: String,
        #[arg(long, default_value = "perf")]
        label: String,
        #[arg(long)]
        quantizer: Option<String>,
    },
}

fn overlap(stacking: Option<&str>) -> Result<bool> {
    match stacking {
        None => Ok(av_export::svg::SvgOptions::default().overlap),
        Some("exact") => Ok(false),
        Some("overlap") => Ok(true),
        _ => Err("stacking must be exact or overlap".into()),
    }
}

fn execute(cli: Cli) -> Result<()> {
    match cli.command {
        Command::ScoreVisual {
            label,
            answers,
            candidate,
        } => visual_score::run(&cli.output_root, &label, &answers, &candidate),
        Command::Profile {
            input,
            config,
            colors,
        } => rnf::profile(&input, &config, colors),
        Command::Rnf { label } => rnf::run(&cli.output_root, &label),
        Command::RnfTrial {
            input,
            config,
            colors,
            output,
        } => rnf::trial(&input, &config, colors, &output),
        Command::Fixture4mp { path } => {
            let scene = std::fs::read_to_string(cli.corpus.join("gt/logo_original_1.svg"))?
                .replacen("width=\"128\"", "width=\"2000\"", 1)
                .replacen("height=\"128\"", "height=\"2000\"", 1);
            let raster = av_metrics::render::svg(&scene, true)?;
            std::fs::write(path, av_session::decode::png_bytes(&raster)?)?;
            Ok(())
        }
        Command::SessionPerf {
            input,
            preview_pixels,
            label,
        } => runner::session_perf(&input, &cli.output_root, &label, preview_pixels),
        Command::CompareRenders {
            reference,
            rendered,
            label,
        } => report::render_check(&reference, &rendered, &cli.output_root, &label),
        Command::GateQuality {
            candidate,
            reference,
        } => report::quality_gate(
            &runner::load_report(&candidate)?,
            &runner::load_report(&reference)?,
        ),
        Command::Visual { a, b, label, seed } => {
            report::visual(&cli.output_root, &a, &b, &label, seed)
        }
        Command::GateCurves {
            candidate,
            reference,
        } => report::curve_gate(
            &runner::load_report(&candidate)?,
            &runner::load_report(&reference)?,
        ),
        Command::Gen { full } => {
            if full {
                full_corpus::generate(&cli.corpus)
            } else {
                corpus::generate(&cli.corpus)
            }
        }
        Command::Run {
            stacking,
            config,
            label,
            quantizer,
            check_baseline,
        } => {
            let result = runner::run(
                &cli.corpus,
                &cli.output_root,
                &label,
                &config,
                quantizer.as_deref(),
                false,
                overlap(stacking.as_deref())?,
            )?;
            if let Some(path) = check_baseline {
                report::check(&runner::load_report(&path)?, &result)?;
            }
            Ok(())
        }
        Command::Baseline {
            stacking,
            engine,
            config,
            label,
        } => {
            if engine != "vtracer" {
                return Err("only vtracer baseline is supported".into());
            }
            runner::run(
                &cli.corpus,
                &cli.output_root,
                &label,
                &config,
                None,
                true,
                overlap(stacking.as_deref())?,
            )?;
            Ok(())
        }
        Command::Compare { a, b, check } => report::compare(&cli.output_root, &a, &b, check),
        Command::Perf {
            stacking,
            config,
            label,
            quantizer,
        } => runner::perf(
            &cli.corpus,
            &cli.output_root,
            &label,
            &config,
            quantizer.as_deref(),
            overlap(stacking.as_deref())?,
        ),
    }
}

fn main() -> std::process::ExitCode {
    match execute(Cli::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("av-bench: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
