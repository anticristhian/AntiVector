//! Exercise the real executable rather than only its decoding helper.

use std::{path::PathBuf, process::Command};

#[test]
fn info_reports_dimensions_alpha_and_errors() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/f1-smoke");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("info.png");
    image::RgbaImage::from_pixel(2, 1, image::Rgba([255, 0, 0, 128]))
        .save(&path)
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_antivector"))
        .arg("info")
        .arg(&path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "width=2 height=1 has_alpha=true"
    );
    println!(
        "antivector info: {}",
        String::from_utf8_lossy(&output.stdout).trim()
    );
    let limited = Command::new(env!("CARGO_BIN_EXE_antivector"))
        .arg("info")
        .arg(&path)
        .args(["--max-pixels", "1"])
        .output()
        .unwrap();
    assert_eq!(limited.status.code(), Some(2));
    let missing = Command::new(env!("CARGO_BIN_EXE_antivector"))
        .arg("info")
        .arg(root.join("missing.png"))
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(4));
    let corrupt = root.join("corrupt.png");
    std::fs::write(&corrupt, b"corrupt").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_antivector"))
        .arg("info")
        .arg(corrupt)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
}
