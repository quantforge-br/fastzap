pub mod accounts;
pub mod app;
pub mod autostart;
pub mod build_info;
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

use commands::{accounts as acc, app as appcmd, bridge, settings as set, webview as wv};
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

pub const APP_ID: &str = "io.github.quantmind_br.fastzap";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let options = app::cli::parse(std::env::args().skip(1));
    #[cfg(target_os = "linux")]
    if let Some(backend) = app::cli::gdk_backend(
        &options,
        std::env::var("GDK_BACKEND").ok().as_deref(),
        app::cli::early_settings().system.display_backend,
    ) {
        // SAFETY: no other thread exists yet; GTK reads this when it starts.
        unsafe { std::env::set_var("GDK_BACKEND", backend) };
    }

    let builder = tauri::Builder::default()
        // Must be the first plugin: a second launch forwards its arguments
        // here, so two processes never write the same settings and accounts.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            app::lifecycle::on_second_instance(app, args);
        }))
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .with_filter(|label| label == app::window::MAIN_WINDOW)
                .build(),
        )
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(notifications::NotificationService::default())
        .manage(app::lifecycle::Services::default())
        .on_menu_event(bridge::on_menu_event);
    #[cfg(not(target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(autostart::ARGS.to_vec()),
    ));
    let app = builder
        .setup(move |app| {
            let handle = app.handle();
            app::bootstrap::init_state(handle)?;
            let state = handle.state::<app::state::AppState>();
            let started_with_system = state.settings.get().system.start_system;
            if !options.set_settings.is_empty() {
                state.settings.update(|s| {
                    app::cli::apply_set_settings(&options, s);
                })?;
            }
            let settings = state.settings.get();
            i18n::set_language(&settings.system.interface_language);
            app::window::create_main(handle)?;
            state.webviews.sync(handle)?;
            tray::init(handle)?;
            #[cfg(unix)]
            app::lifecycle::install_signal_handler(handle);
            #[cfg(target_os = "linux")]
            {
                app::lifecycle::start_dbus(handle);
                app::theme::monitor_system(handle);
            }
            // Keep the entry pointing at this executable (e.g. after an
            // update moved it); turning it off is left to the preference,
            // including a change made by `--setSettings` on this launch.
            if settings.system.start_system || started_with_system {
                autostart::sync(handle, settings.system.start_system);
            }
            app::panel::init(handle)?;
            app::actions::apply_scale(handle);
            let hidden = app::lifecycle::starts_hidden(
                &options,
                settings.system.start_background,
                settings.onboarding.completed,
            );
            if hidden {
                app::panel::sync(handle);
            } else {
                app::window::show(handle);
            }
            for link in &options.links {
                if app::lifecycle::open_link(handle, link) {
                    break;
                }
            }
            app::lifecycle::mark_ready(handle);
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
            wv::app_close_confirmed,
            wv::app_desktop,
            wv::unread_get,
            wv::account_sync_photo,
            wv::calling_codes,
            wv::open_chat_by_number,
            set::app_locale,
            appcmd::app_info,
            appcmd::panel_supports_positioning,
            appcmd::proxy_password_set,
            appcmd::proxy_password_present,
            appcmd::settings_reset,
            appcmd::dialog_pick_folder,
            appcmd::dialog_pick_files,
            appcmd::open_path,
            appcmd::reveal_path,
            appcmd::downloads_default_dir,
            bridge::wa_title_changed,
            bridge::wa_shortcut,
            bridge::wa_theme_event,
            bridge::wa_open_account_selector,
            bridge::wa_profile_photo,
            bridge::wa_notify,
            bridge::wa_notification_close,
        ])
        .build(tauri::generate_context!())
        .expect("error while building FastZap");
    app.run(|handle, event| match event {
        // Windows still exist here, unlike at `Exit`.
        tauri::RunEvent::ExitRequested { .. } => app::panel::shutdown(handle),
        tauri::RunEvent::Exit => app::lifecycle::shutdown(handle),
        // macOS delivers `whatsapp:` links as Apple events, not arguments.
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => {
            for url in urls {
                if app::lifecycle::open_link(handle, url.as_str()) {
                    break;
                }
            }
        }
        _ => {}
    });
}
