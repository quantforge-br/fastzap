pub mod accounts;
pub mod app;
pub mod commands;
pub mod migration;
pub mod paths;
pub mod secrets;
pub mod settings;

use commands::{accounts as acc, settings as set};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app::bootstrap::init_state(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set::settings_get,
            set::settings_patch,
            acc::accounts_list,
            acc::accounts_create,
            acc::accounts_update,
            acc::accounts_delete,
            acc::accounts_reorder,
            acc::user_agent_presets,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FastZap");
}
