//! Exercise all formats, direct RF11 syntax, batch isolation and exit codes.
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};
fn root(name: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/f6-cli-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_antivector"))
        .args(args)
        .output()
        .unwrap()
}
fn fixture(path: &Path) {
    let mut image = image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
    for y in 2..6 {
        for x in 2..6 {
            image.put_pixel(x, y, image::Rgba([0, 0, 255, 255]));
        }
    }
    image.save(path).unwrap();
}

#[test]
fn five_formats_and_direct_cli_syntax() {
    let root = root("formats");
    let input = root.join("source.png");
    fixture(&input);
    for (ext, magic) in [
        ("svg", b"<svg".as_slice()),
        ("pdf", b"%PDF-"),
        ("eps", b"%!PS-Adobe"),
        ("dxf", b"0\nSECTION"),
        ("png", b"\x89PNG\r\n\x1a\n"),
    ] {
        let output = root.join(format!("result.{ext}"));
        let result = run(&[
            input.as_os_str(),
            "-o".as_ref(),
            output.as_os_str(),
            "--preset".as_ref(),
            "pixelart".as_ref(),
        ]);
        assert!(
            result.status.success(),
            "{ext}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(std::fs::read(output).unwrap().starts_with(magic));
    }
    let params = root.join("params.json");
    std::fs::write(&params, include_str!("../../av-core/presets/logo.json")).unwrap();
    let output = root.join("override.bin");
    let result = run(&[
        "vectorize".as_ref(),
        input.as_os_str(),
        "-o".as_ref(),
        output.as_os_str(),
        "--format".as_ref(),
        "pdf".as_ref(),
        "--params".as_ref(),
        params.as_os_str(),
    ]);
    assert!(result.status.success());
    assert!(std::fs::read(output).unwrap().starts_with(b"%PDF-"));
    let svg = root.join("group.svg");
    let result = run(&[
        input.as_os_str(),
        "-o".as_ref(),
        svg.as_os_str(),
        "--group-by-color".as_ref(),
        "--precision".as_ref(),
        "4".as_ref(),
    ]);
    assert!(result.status.success());
}

#[test]
fn batch_continues_after_corruption_and_rejects_collisions() {
    let root = root("batch");
    let input = root.join("in");
    let output = root.join("out");
    std::fs::create_dir_all(&input).unwrap();
    fixture(&input.join("a.png"));
    std::fs::write(input.join("b.png"), b"broken image").unwrap();
    fixture(&input.join("z.png"));
    std::fs::write(input.join("notes.txt"), b"ignored").unwrap();
    let result = run(&[
        "batch".as_ref(),
        input.as_os_str(),
        "-o".as_ref(),
        output.as_os_str(),
        "--format".as_ref(),
        "pdf".as_ref(),
    ]);
    assert_eq!(result.status.code(), Some(1));
    assert!(output.join("a.pdf").is_file());
    assert!(output.join("z.pdf").is_file());
    assert!(!output.join("b.pdf").exists());
    assert!(String::from_utf8_lossy(&result.stdout).contains("succeeded=2 failed=1"));
    assert!(String::from_utf8_lossy(&result.stderr).contains("exit 3"));
    // A second file with the same stem must not overwrite a previous conversion.
    // Different extensions remain distinct even on case-insensitive APFS/NTFS.
    // The stems also exercise portable case folding on case-sensitive Unix.
    let jpeg = input.join("A.jpg");
    image::RgbImage::from_pixel(1, 1, image::Rgb([1, 2, 3]))
        .save(&jpeg)
        .unwrap();
    let result = run(&[
        "batch".as_ref(),
        input.as_os_str(),
        "-o".as_ref(),
        output.as_os_str(),
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("collision"));
}

#[test]
fn invalid_export_options_and_io_are_reported_before_writing() {
    let root = root("errors");
    let input = root.join("source.png");
    fixture(&input);
    let before = std::fs::read(&input).unwrap();
    let same = run(&[input.as_os_str(), "-o".as_ref(), input.as_os_str()]);
    assert_eq!(same.status.code(), Some(2));
    assert_eq!(before, std::fs::read(&input).unwrap());
    let output = root.join("result.svg");
    for args in [
        vec!["--precision", "9"],
        vec!["--group-by-color", "--stacking", "overlap"],
        vec!["--colors", "1"],
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_antivector"));
        let result = command
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(!output.exists());
    }
    let unknown = root.join("out.unknown");
    assert_eq!(
        run(&[input.as_os_str(), "-o".as_ref(), unknown.as_os_str()])
            .status
            .code(),
        Some(2)
    );
    let missing = root.join("absent/out.svg");
    assert_eq!(
        run(&[input.as_os_str(), "-o".as_ref(), missing.as_os_str()])
            .status
            .code(),
        Some(4)
    );
}
