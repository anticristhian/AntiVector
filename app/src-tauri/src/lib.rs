//! AntiVector desktop shell; heavy work stays in av-session workers.
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            commands::export
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
