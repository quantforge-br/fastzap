pub mod accounts;
pub mod app;
pub mod commands;
pub mod customizations;
pub mod downloads;
pub mod i18n;
pub mod migration;
pub mod notifications;
pub mod paths;
pub mod platform;
pub mod proxy;
pub mod secrets;
pub mod settings;
pub mod tray;
pub mod webview;

use commands::{accounts as acc, bridge, settings as set, webview as wv};
use tauri::Manager;

pub const APP_ID: &str = "io.github.quantmind_br.fastzap";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        // Must be the first plugin: a second launch only focuses this instance,
        // so two processes never write the same settings and accounts.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            app::window::show(app);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(notifications::NotificationService::default())
        .setup(|app| {
            let handle = app.handle();
            app::bootstrap::init_state(handle)?;
            let state = handle.state::<app::state::AppState>();
            let settings = state.settings.get();
            i18n::set_language(&settings.system.interface_language);
            app::window::create_main(handle)?;
            state.webviews.sync(handle)?;
            tray::init(handle)?;
            app::window::show(handle);
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
            wv::layout_get,
            wv::layout_set_bounds,
            wv::layout_set_overlay,
            wv::account_activate,
            wv::app_action,
            wv::unread_get,
            wv::account_sync_photo,
            wv::calling_codes,
            wv::open_chat_by_number,
            bridge::wa_title_changed,
            bridge::wa_shortcut,
            bridge::wa_theme_event,
            bridge::wa_open_account_selector,
            bridge::wa_profile_photo,
            bridge::wa_notify,
            bridge::wa_notification_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FastZap");
}
