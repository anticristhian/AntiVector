//! Reproducible quality harness; vtracer is confined to this binary.

mod corpus;
mod report;
mod runner;

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
    /// Check the measured geometric/compactness acceptance criteria of F4.
    GateCurves {
        candidate: PathBuf,
        reference: PathBuf,
    },
    /// Generate original fixtures with SVG ground truth and license manifest.
    Gen,
    /// Measure AntiVector using a preset or complete parameter JSON file.
    Run {
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
        #[arg(long, default_value = "logo")]
        config: String,
        #[arg(long, default_value = "perf")]
        label: String,
        #[arg(long)]
        quantizer: Option<String>,
    },
}

fn execute(cli: Cli) -> Result<()> {
    match cli.command {
        Command::GateCurves {
            candidate,
            reference,
        } => report::curve_gate(
            &runner::load_report(&candidate)?,
            &runner::load_report(&reference)?,
        ),
        Command::Gen => corpus::generate(&cli.corpus),
        Command::Run {
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
            )?;
            if let Some(path) = check_baseline {
                report::check(&runner::load_report(&path)?, &result)?;
            }
            Ok(())
        }
        Command::Baseline {
            engine,
            config,
            label,
        } => {
            if engine != "vtracer" {
                return Err("only vtracer baseline is supported".into());
            }
            runner::run(&cli.corpus, &cli.output_root, &label, &config, None, true)?;
            Ok(())
        }
        Command::Compare { a, b, check } => report::compare(&cli.output_root, &a, &b, check),
        Command::Perf {
            config,
            label,
            quantizer,
        } => runner::perf(
            &cli.corpus,
            &cli.output_root,
            &label,
            &config,
            quantizer.as_deref(),
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
