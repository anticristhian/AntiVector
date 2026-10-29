//! AntiVector desktop shell; heavy work stays in av-session workers.
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .on_page_load(|_, payload| {
            // Local CI smoke hook, opt-in via environment; no telemetry or UI.
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                if let Some(path) = std::env::var_os("AV_STARTUP_MARKER_PATH") {
                    let _ = std::fs::write(path, b"embedded-webview-page-finished\n");
                }
            }
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(std::sync::Arc::new(commands::DesktopState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::open_image,
            commands::open_image_bytes,
            commands::open_clipboard,
            commands::record_ui_metrics,
            commands::vectorize,
            commands::cancel,
            commands::close_session,
            commands::edit,
            commands::undo,
            commands::redo,
            commands::save_project,
            commands::open_project,
            commands::open_project_bytes,
            commands::current_document,
            commands::estimate_fidelity,
            commands::export
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
