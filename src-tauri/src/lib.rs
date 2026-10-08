mod commands;
pub(crate) mod utils;
mod watcher;

pub use utils::is_running_in_vm;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin registered, same as soplos-welcome-3.0 and
        // soplos-webapp-manager-3.0: it needs to intercept and exit a second
        // launch before anything else runs.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            watcher::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::appimages::list_installed,
            commands::appimages::pick_appimage_file,
            commands::appimages::add_appimage,
            commands::appimages::integrate_existing,
            commands::appimages::run_appimage,
            commands::appimages::delete_appimage,
            commands::appimages::update_appimage_info,
            commands::appimages::read_icon_preview,
            commands::updates::get_update_mode,
            commands::updates::check_for_update_api,
            commands::updates::check_for_update_direct,
            commands::updates::download_update,
            commands::updates::check_for_update_tool,
            commands::updates::run_update_tool,
            commands::environment::get_environment_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Soplos AppImage Manager");
}
