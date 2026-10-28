use std::{path::PathBuf, process::Command};

#[test]
fn polygon_svg_and_stage_dumps() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/f2-smoke");
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("logo.png");
    let output = root.join("logo.svg");
    let stages = root.join("stages");
    let mut image = image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
    for y in 2..6 {
        for x in 2..6 {
            image.put_pixel(x, y, image::Rgba([0, 0, 255, 255]));
        }
    }
    image.save(&input).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_antivector"))
        .arg("vectorize")
        .arg(input)
        .arg("-o")
        .arg(&output)
        .args(["--colors", "2", "--dump-stages"])
        .arg(&stages)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let svg = std::fs::read_to_string(output).unwrap();
    assert_eq!(svg.matches("<path ").count(), 2);
    assert!(svg.contains("fill-rule=\"evenodd\""));
    for name in ["quantize", "label", "merge_small"] {
        assert_eq!(
            image::open(stages.join(format!("{name}.png")))
                .unwrap()
                .width(),
            8
        );
    }
    println!(
        "polygon CLI: {}",
        String::from_utf8_lossy(&result.stdout).trim()
    );
}
