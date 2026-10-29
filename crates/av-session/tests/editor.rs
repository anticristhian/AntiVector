use av_core::{edit::EditOp, ColorCount, Control, Params, RgbaImage};
use av_session::editor::Editor;
use std::sync::Arc;

fn make_editor() -> Editor {
    let image = RgbaImage::new(
        12,
        12,
        (0..144)
            .map(|i| {
                if i % 12 < 6 {
                    [20, 40, 80, 255]
                } else {
                    [220, 70, 30, 255]
                }
            })
            .collect(),
    )
    .unwrap();
    Editor::new(Arc::new(image)).unwrap()
}
fn params() -> Params {
    let mut p = Params {
        min_region_area: 2,
        colors: ColorCount::Fixed(2),
        ..Params::default()
    };
    p.experimental.quality_v5 = false;
    p.experimental.curve_fit = false;
    p
}
fn svg(doc: &av_core::VectorDoc) -> String {
    av_export::svg::to_string(doc).unwrap()
}

#[test]
fn history_restores_exact_svg_over_100_edits_and_branches() {
    let ctl = Control::default();
    let mut editor = make_editor();
    let first = editor.draft_params(&params(), &ctl).unwrap();
    editor.commit(first);
    let mut snapshots = vec![svg(&editor.current().unwrap().doc)];
    for i in 0..110 {
        let draft = editor
            .draft_edit(
                EditOp::RecolorPalette {
                    color: 0,
                    rgb: [30 + i, 40, 80],
                },
                &ctl,
            )
            .unwrap();
        editor.commit(draft);
        snapshots.push(svg(&editor.current().unwrap().doc));
    }
    assert_eq!(editor.history().undo_depth, 110);
    for expected in snapshots[..110].iter().rev() {
        assert_eq!(svg(&editor.undo().unwrap().doc), *expected);
    }
    for expected in &snapshots[1..] {
        assert_eq!(svg(&editor.redo().unwrap().doc), *expected);
    }
    editor.undo().unwrap();
    let draft = editor
        .draft_edit(EditOp::DeletePalette { color: 1 }, &ctl)
        .unwrap();
    editor.commit(draft);
    assert_eq!(editor.history().redo_depth, 0);
    assert!(editor.redo().is_err());
}

#[test]
fn parameters_locks_replay_and_failed_drafts_are_transactional() {
    let ctl = Control::default();
    let mut editor = make_editor();
    let initial = editor.draft_params(&params(), &ctl).unwrap();
    let original = svg(&initial.doc);
    editor.commit(initial);
    for op in [
        EditOp::LockColor {
            color: 0,
            locked: true,
        },
        EditOp::RecolorPalette {
            color: 0,
            rgb: [25, 50, 90],
        },
        EditOp::MergeRegion {
            region: 0,
            into: None,
        },
    ] {
        let draft = editor.draft_edit(op, &ctl).unwrap();
        editor.commit(draft);
        let current = editor.current().unwrap();
        let mut reloaded = make_editor();
        let replayed = reloaded
            .draft_replay(&current.params, current.edits.clone(), &ctl)
            .unwrap();
        assert_eq!(svg(&current.doc), svg(&replayed.doc));
    }
    let current = editor.current().unwrap();
    let mut changed = current.params.clone();
    changed.smoothness = 0.8;
    changed.fit_tolerance_px = 1.2;
    let draft = editor.draft_params(&changed, &ctl).unwrap();
    editor.commit(draft);
    assert_eq!(editor.undo().unwrap().params, current.params);
    assert_eq!(editor.redo().unwrap().params, changed);
    let current = editor.current().unwrap();
    let history = editor.history();
    assert!(editor
        .draft_edit(EditOp::DeletePalette { color: 999 }, &ctl)
        .is_err());
    let cancelled = Control::default();
    cancelled
        .cancel
        .store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(editor.draft_params(&params(), &cancelled).is_err());
    assert_eq!(editor.history().undo_depth, history.undo_depth);
    assert_eq!(svg(&editor.current().unwrap().doc), svg(&current.doc));
    while editor.history().undo_depth > 0 {
        editor.undo().unwrap();
    }
    assert_eq!(svg(&editor.current().unwrap().doc), original);
}
#[test]
fn session_edit_events_and_history_publish_together() {
    use av_session::{JobEvent, Session};
    use std::{sync::mpsc, time::Duration};
    let image = RgbaImage::new(2, 1, vec![[20, 40, 80, 255], [220, 70, 30, 255]]).unwrap();
    let session = Arc::new(Session::new(image).unwrap());
    let first = session.vectorize(&params(), &Control::default()).unwrap();
    let (tx, rx) = mpsc::channel();
    let id = session
        .start_edit(
            EditOp::RecolorPalette {
                color: 0,
                rgb: [40, 80, 160],
            },
            Arc::new(move |event| {
                tx.send(event).unwrap();
            }),
        )
        .unwrap();
    let completed = loop {
        match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
            JobEvent::Done { job_id, doc } => {
                assert_eq!(job_id, id);
                break doc;
            }
            JobEvent::Error { error, .. } => panic!("{error}"),
            _ => {}
        }
    };
    assert_eq!(session.history().unwrap().undo_depth, 1);
    assert!(!Arc::ptr_eq(&first, &completed));
    assert!(Arc::ptr_eq(&first, &session.undo().unwrap().doc));
    assert!(Arc::ptr_eq(&completed, &session.redo().unwrap().doc));
}
#[test]
fn region_removal_does_not_expand_when_a_parameter_change_merges_regions() {
    let source = RgbaImage::new(
        8,
        1,
        vec![
            [20, 40, 80, 255],
            [20, 40, 80, 255],
            [80, 100, 140, 255],
            [80, 100, 140, 255],
            [255, 255, 255, 255],
            [255, 255, 255, 255],
            [255, 255, 255, 255],
            [255, 255, 255, 255],
        ],
    )
    .unwrap();
    let mut editor = Editor::new(Arc::new(source)).unwrap();
    let mut p = params();
    p.colors = ColorCount::Fixed(3);
    let first = editor.draft_params(&p, &Control::default()).unwrap();
    editor.commit(first);
    let removed = editor
        .draft_edit(EditOp::RemoveRegion { region: 0 }, &Control::default())
        .unwrap();
    editor.commit(removed);
    p.colors = ColorCount::Fixed(2);
    let changed = editor.draft_params(&p, &Control::default()).unwrap();
    let png = av_export::png::to_bytes(&changed.doc, &Default::default()).unwrap();
    let pixels = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(pixels.get_pixel(0, 0)[3], 0);
    assert_eq!(pixels.get_pixel(1, 0)[3], 0);
    assert_eq!(
        pixels.get_pixel(2, 0)[3],
        255,
        "a collapsed label must not enlarge a region erasure"
    );
    assert_eq!(pixels.get_pixel(3, 0)[3], 255);
}
