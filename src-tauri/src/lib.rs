pub mod accounts;
pub mod app;
pub mod autostart;
pub mod build_info;
pub mod commands;
pub mod customizations;
pub mod donation;
pub mod downloads;
pub mod evolution;
pub mod i18n;
pub mod migration;
pub mod notifications;
pub mod paths;
pub mod permissions;
pub mod platform;
pub mod proxy;
pub mod reporting;
pub mod secrets;
pub mod settings;
pub mod spellcheck;
pub mod tray;
pub mod update;
pub mod webview;

use commands::{
    accounts as acc, app as appcmd, bridge, customizations as custom, downloads as dl,
    evolution as evocmd, reporting as report, settings as set, spellcheck as spell, webview as wv,
};
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

pub const APP_ID: &str = "io.github.quantmind_br.fastzap";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    reporting::session::TeeLogger::init(
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).build(),
    );
    reporting::session::install_panic_hook();
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
    #[cfg(target_os = "linux")]
    if platform::disable_dmabuf_renderer(
        platform::is_flatpak(),
        platform::has_nvidia_driver(),
        std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER")
            .ok()
            .as_deref(),
    ) {
        // SAFETY: still single-threaded; WebKitGTK reads this when it starts.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
    // Enchant reads its Hunspell dictionaries from here; `AppPaths` resolves
    // the same directory once Tauri runs.
    #[cfg(target_os = "linux")]
    if let Some(data) = dirs::data_dir() {
        // SAFETY: still single-threaded, as above.
        unsafe {
            std::env::set_var(
                "ENCHANT_CONFIG_DIR",
                data.join(APP_ID).join(paths::DICTIONARIES_DIR),
            )
        };
    }

    let builder = tauri::Builder::default()
        // Must be the first plugin: a second launch forwards its arguments
        // here, so two processes never write the same settings and accounts.
        // The D-Bus name must start with the app ID (Flatpak only lets an app
        // own names under its ID), not the bundle identifier.
        .plugin(
            tauri_plugin_single_instance::Builder::new()
                .callback(|app, args, _cwd| app::lifecycle::on_second_instance(app, args))
                .dbus_id(APP_ID)
                .build(),
        )
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
        .manage(evolution::Evolution::default())
        .manage(update::UpdateState::default())
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
            reporting::session::start(
                &reporting::store::ReportStore::new(state.paths.reports_dir()),
                state.settings.get().reporting.crash_prompts,
            );
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
            evolution::sync(handle);
            spellcheck::provision(handle);
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
            reporting::session::mark("running");
            update::start(handle);
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
            acc::account_photo_pick,
            dl::downloads_list,
            dl::download_answer,
            dl::downloads_clear_history,
            dl::download_remove,
            dl::download_delete_file,
            dl::download_open,
            dl::download_reveal,
            appcmd::permission_answer,
            custom::custom_list,
            custom::custom_read,
            custom::custom_write,
            custom::custom_import_file,
            custom::custom_import_url,
            custom::custom_duplicate,
            custom::custom_delete,
            custom::custom_set_file_enabled,
            custom::custom_open_folder,
            custom::custom_pick_preview,
            custom::custom_preview,
            custom::custom_reload,
            spell::spell_state,
            spell::spell_set_languages,
            spell::spell_install,
            spell::spell_remove,
            spell::spell_import,
            spell::spell_open_folder,
            report::report_preview,
            report::report_save,
            report::report_open_github,
            report::report_list,
            report::report_pending,
            report::report_keep,
            report::report_delete,
            report::report_clear,
            report::report_open_folder,
            appcmd::update_info,
            appcmd::donation_open,
            appcmd::donation_open_website,
            report::report_copied,
            evocmd::evo_instances,
            evocmd::evo_setup,
            evocmd::evo_connection,
            evocmd::evo_reconnect,
            evocmd::evo_qr,
            evocmd::evo_pair_phone,
            evocmd::evo_logout,
            evocmd::evo_chats,
            evocmd::evo_messages,
            evocmd::evo_set_open_chat,
            evocmd::evo_mark_read,
            evocmd::evo_typing,
            evocmd::evo_send_text,
            evocmd::evo_retry,
            evocmd::evo_send_file,
            evocmd::evo_react,
            evocmd::evo_edit,
            evocmd::evo_delete,
            evocmd::evo_media,
            evocmd::evo_save_media,
            evocmd::evo_load_older,
            evocmd::evo_avatar,
            evocmd::evo_refresh_contacts,
            evocmd::evo_info,
            evocmd::evo_open_passkey,
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
        tauri::RunEvent::ExitRequested { .. } => {
            reporting::session::mark("shutdown");
            app::panel::shutdown(handle);
        }
        tauri::RunEvent::Exit => {
            app::lifecycle::shutdown(handle);
            reporting::session::close();
        }
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
