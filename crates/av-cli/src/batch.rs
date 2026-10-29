//! Deterministic directory conversion, isolating individual image failures.
use crate::vectorize::{Failure, Settings, VectorizeArgs};
use std::{collections::BTreeSet, path::PathBuf};

#[derive(clap::Args)]
pub struct BatchArgs {
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: PathBuf,
    #[command(flatten)]
    pub settings: Settings,
}

pub fn run(args: BatchArgs) -> Result<u8, Failure> {
    args.settings.options()?;
    args.settings.parameters()?;
    let extension = args.settings.format.as_deref().unwrap_or("svg");
    let mut files = Vec::new();
    let mut targets = BTreeSet::new();
    for entry in std::fs::read_dir(&args.input).map_err(|e| (4, e.to_string()))? {
        let entry = entry.map_err(|e| (4, e.to_string()))?;
        if !entry.file_type().map_err(|e| (4, e.to_string()))?.is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "bmp" | "gif" | "tif" | "tiff" | "webp"
        ) {
            continue;
        }
        let stem = path
            .file_stem()
            .ok_or_else(|| (2, "missing filename".into()))?;
        let mut output = args.output.join(stem);
        output.set_extension(extension);
        // Case-fold also on Unix so a batch has portable collision semantics.
        if !targets.insert(output.as_os_str().to_string_lossy().to_lowercase()) {
            return Err((2, format!("batch output collision: {}", output.display())));
        }
        files.push((path, output));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    std::fs::create_dir_all(&args.output).map_err(|e| (4, e.to_string()))?;
    if args.input.canonicalize().map_err(|e| (4, e.to_string()))?
        == args.output.canonicalize().map_err(|e| (4, e.to_string()))?
    {
        return Err((2, "batch input and output directories must differ".into()));
    }
    let mut succeeded = 0;
    let mut failed = 0;
    for (input, output) in files {
        let name = input.display().to_string();
        match crate::vectorize::run(VectorizeArgs {
            input,
            output,
            settings: args.settings.clone(),
            dump_stages: None,
        }) {
            Ok(()) => {
                succeeded += 1;
                println!("ok {name}");
            }
            Err((code, message)) => {
                failed += 1;
                eprintln!("failed {name} (exit {code}): {message}");
            }
        }
    }
    println!("batch: succeeded={succeeded} failed={failed}");
    Ok(if failed == 0 { 0 } else { 1 })
}
