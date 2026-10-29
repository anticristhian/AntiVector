//! Headless AntiVector application entry point.

mod batch;
mod decode;
mod vectorize;

use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(name = "antivector", version, about = "Offline raster-to-vector tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Decode an image and print its dimensions and alpha flag.
    Info {
        path: PathBuf,
        /// Maximum number of decoded pixels (default: 64 MP).
        #[arg(long, default_value_t = av_core::raster::DEFAULT_PIXEL_LIMIT)]
        max_pixels: u64,
    },
    /// Convert an image to SVG, PDF, EPS, DXF or PNG.
    Vectorize(vectorize::VectorizeArgs),
    /// Convert supported files in a directory, continuing after image failures.
    Batch(batch::BatchArgs),
}

fn main() -> ExitCode {
    let mut args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| {
        ![
            "info",
            "vectorize",
            "batch",
            "help",
            "--help",
            "-h",
            "--version",
            "-V",
        ]
        .iter()
        .any(|v| arg == v)
    }) {
        args.insert(1, "vectorize".into());
    }
    let cli = Cli::parse_from(args);
    match cli.command {
        Command::Batch(args) => match batch::run(args) {
            Ok(code) => ExitCode::from(code),
            Err((code, message)) => {
                eprintln!("{message}");
                ExitCode::from(code)
            }
        },
        Command::Vectorize(args) => match vectorize::run(args) {
            Ok(()) => ExitCode::SUCCESS,
            Err((code, message)) => {
                eprintln!("{message}");
                ExitCode::from(code)
            }
        },
        Command::Info { path, max_pixels } => match decode::load(&path, max_pixels) {
            Ok(image) => {
                println!(
                    "width={} height={} has_alpha={}",
                    image.w,
                    image.h,
                    image.has_alpha()
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(error.exit_code())
            }
        },
    }
}
