//! Headless AntiVector application entry point.

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
    /// Convert to SVG (provisional pixel-grid polygons until curve fitting in F4).
    Vectorize(vectorize::VectorizeArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
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
