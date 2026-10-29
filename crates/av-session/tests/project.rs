use av_core::{edit::EditOp, ColorCount, Control, Params, RgbaImage};
use av_session::{editor::Editor, project, Session};
use std::{
    io::{Read, Write},
    sync::Arc,
};
fn temporary() -> tempfile::TempDir {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/f8-project-tests");
    std::fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}
fn source() -> RgbaImage {
    RgbaImage::new(
        3,
        2,
        vec![
            [20, 40, 80, 255],
            [20, 40, 80, 255],
            [220, 60, 30, 255],
            [20, 40, 80, 255],
            [9, 8, 7, 0],
            [220, 60, 30, 255],
        ],
    )
    .unwrap()
}
fn revision() -> av_session::editor::Revision {
    let mut editor = Editor::new(Arc::new(source())).unwrap();
    let mut params = Params {
        min_region_area: 2,
        colors: ColorCount::Fixed(2),
        ..Params::default()
    };
    params.experimental.quality_v5 = false;
    params.experimental.curve_fit = false;
    let initial = editor.draft_params(&params, &Control::default()).unwrap();
    editor.commit(initial);
    for op in [
        EditOp::LockColor {
            color: 0,
            locked: true,
        },
        EditOp::RecolorPalette {
            color: 0,
            rgb: [40, 80, 160],
        },
        EditOp::RemoveRegion { region: 1 },
    ] {
        let next = editor.draft_edit(op, &Control::default()).unwrap();
        editor.commit(next);
    }
    editor.current().unwrap()
}
#[test]
fn project_roundtrip_reproduces_pixels_params_edits_and_svg() {
    let temp = temporary();
    let path = temp.path().join("example.avproj");
    let revision = revision();
    project::save(&path, "ação.png", &source(), &revision).unwrap();
    let loaded = project::open(&path).unwrap();
    assert_eq!(loaded.image, source());
    assert_eq!(loaded.params, revision.params);
    assert_eq!(loaded.edits, revision.edits);
    assert_eq!(loaded.name, "ação.png");
    let restored = Session::new(loaded.image)
        .unwrap()
        .restore(&loaded.params, loaded.edits, &Control::default())
        .unwrap();
    assert_eq!(
        av_export::svg::to_string(&revision.doc).unwrap(),
        av_export::svg::to_string(&restored.doc).unwrap()
    );
    let bytes = std::fs::read(&path).unwrap();
    let mut invalid = revision.clone();
    invalid.params.smoothness = f32::NAN;
    assert!(project::save(&path, "invalid", &source(), &invalid).is_err());
    assert_eq!(
        bytes,
        std::fs::read(&path).unwrap(),
        "failed save must preserve existing project"
    );
    project::save(&path, "updated.png", &source(), &revision).unwrap();
    assert_eq!(project::open(&path).unwrap().name, "updated.png");
}
#[test]
fn unknown_versions_paths_and_oversized_directories_are_rejected() {
    let temp = temporary();
    let path = temp.path().join("source.avproj");
    project::save(&path, "test.png", &source(), &revision()).unwrap();
    for kind in ["version", "path", "size", "selection"] {
        let mut input = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let altered = temp.path().join(format!("{kind}.avproj"));
        let mut output = zip::ZipWriter::new(std::fs::File::create(&altered).unwrap());
        for i in 0..input.len() {
            let mut entry = input.by_index(i).unwrap();
            let mut name = entry.name().unwrap().into_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            if kind == "version" && name == "manifest.json" {
                let mut m: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                m["version"] = 2.into();
                bytes = serde_json::to_vec(&m).unwrap();
            }
            if kind == "path" && name == "original.png" {
                name = "../original.png".into();
            }
            if kind == "size" && name == "params.json" {
                bytes = vec![b' '; 64_001];
            }
            if kind == "selection" && name == "edits.json" {
                let mut ops: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                ops[0]["from"]["point"] = serde_json::json!([-1, 0]);
                bytes = serde_json::to_vec(&ops).unwrap();
            }
            output
                .start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            output.write_all(&bytes).unwrap();
        }
        output.finish().unwrap();
        if kind == "selection" {
            let loaded = project::open(&altered).unwrap();
            assert!(Session::new(loaded.image)
                .unwrap()
                .restore(&loaded.params, loaded.edits, &Control::default())
                .is_err());
        } else {
            assert!(project::open(&altered).is_err(), "{kind}");
        }
    }
    let malformed = temp.path().join("malformed.avproj");
    let mut bytes = std::fs::read(&path).unwrap();
    let len = bytes.len();
    bytes[len - 12..len - 10].copy_from_slice(&u16::MAX.to_le_bytes());
    std::fs::write(&malformed, bytes).unwrap();
    assert!(project::open(&malformed).is_err());
    assert!(!temp.path().join("../original.png").exists());
}
