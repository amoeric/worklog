#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use worklog_app::commands;

fn main() {
    // 舊版的「待寫回」已經拿掉了，設定目錄裡若還有 pending.json 就收起來
    worklog_app::store::retire_pending_file();

    // 狀態表（內建八個 ＋ 使用者自己加的）先灌進去，解析與寫檔兩邊才認得
    worklog_app::model::set_table(worklog_app::store::load_statuses());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // 線上更新：設定在 tauri.conf.json 的 plugins.updater
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
            commands::status_table,
            commands::add_status,
            commands::delete_status,
            commands::rules_with_status,
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
