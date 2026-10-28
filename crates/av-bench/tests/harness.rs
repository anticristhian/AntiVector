use std::{path::PathBuf, process::Command};

#[test]
fn harness_end_to_end_and_regression_gate() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/f3-smoke");
    let corpus = root.join("corpus");
    let output = root.join("out");
    let run = |args: &[&str]| {
        let result = Command::new(env!("CARGO_BIN_EXE_av-bench"))
            .arg("--corpus")
            .arg(&corpus)
            .arg("--output-root")
            .arg(&output)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{args:?}\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    };
    run(&["gen"]);
    let manifest = corpus.join("manifest.json");
    let entries: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    assert_eq!(entries.len(), 34);
    let jpeg = image::open(corpus.join("images/jpeg_q60.jpg"))
        .unwrap()
        .to_rgba8();
    let raster = av_core::RgbaImage::new(
        jpeg.width(),
        jpeg.height(),
        jpeg.pixels().map(|p| p.0).collect(),
    )
    .unwrap();
    assert!(av_core::detect(&raster).colors.abs_diff(2) <= 1);
    let subset = vec![entries[0].clone(), entries[20].clone(), entries[26].clone()];
    std::fs::write(&manifest, serde_json::to_vec(&subset).unwrap()).unwrap();
    run(&["run", "--label", "a"]);
    run(&["baseline", "vtracer", "--label", "b"]);
    run(&["compare", "a", "b"]);
    run(&["visual", "a", "b", "--label", "smoke"]);
    let answers: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("visual/smoke.json")).unwrap()).unwrap();
    assert_eq!(answers["status"], "pending_human");
    assert!(answers["answers"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["choice"].is_null()));
    run(&["perf"]);
    assert!(output.join("compare-a-b/report.html").is_file());
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("a/metrics.json")).unwrap()).unwrap();
    assert_eq!(report["rows"].as_array().unwrap().len(), 3);
    let check = Command::new(env!("CARGO_BIN_EXE_av-bench"))
        .arg("--corpus")
        .arg(&corpus)
        .arg("--output-root")
        .arg(&output)
        .args(["run", "--label", "c", "--check-baseline"])
        .arg(output.join("a/metrics.json"))
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    let mut broken = report.clone();
    broken["rows"][0]["ssim"] = 1.5.into();
    std::fs::write(
        output.join("broken.json"),
        serde_json::to_vec(&broken).unwrap(),
    )
    .unwrap();
    let check = Command::new(env!("CARGO_BIN_EXE_av-bench"))
        .arg("--corpus")
        .arg(&corpus)
        .arg("--output-root")
        .arg(&output)
        .args(["run", "--label", "d", "--check-baseline"])
        .arg(output.join("broken.json"))
        .output()
        .unwrap();
    assert!(!check.status.success());
}
