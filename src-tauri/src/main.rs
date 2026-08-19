#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use worklog_app::commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // 線上更新：設定在 tauri.conf.json 的 plugins.updater
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::load_settings,
            commands::save_settings,
            commands::save_external_settings,
            commands::clear_token,
            commands::load_rules,
            commands::save_rules,
            commands::fetch_link,
            commands::fetch_image,
            commands::pick_folder,
            commands::load_workspace,
            commands::move_item,
            commands::clear_pending,
            commands::status_table,
            commands::load_todos,
            commands::save_todos,
            commands::open_url,
            commands::open_log_file,
            commands::append_entry,
            commands::app_version,
            commands::check_update,
            commands::install_update,
            commands::restart_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
