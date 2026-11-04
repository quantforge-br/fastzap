use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Commands exposed to the local UI.
const UI_COMMANDS: &[&str] = &[
    "settings_get",
    "settings_patch",
    "accounts_list",
    "accounts_create",
    "accounts_update",
    "accounts_delete",
    "accounts_reorder",
    "user_agent_presets",
    "layout_get",
    "layout_set_bounds",
    "layout_set_overlay",
    "account_activate",
    "app_action",
    "app_close_confirmed",
    "app_desktop",
    "unread_get",
    "account_sync_photo",
    "calling_codes",
    "open_chat_by_number",
    "app_locale",
    "app_info",
    "panel_supports_positioning",
    "proxy_password_set",
    "proxy_password_present",
    "settings_reset",
    "dialog_pick_folder",
    "dialog_pick_files",
    "open_path",
    "reveal_path",
    "downloads_default_dir",
    "account_photo_pick",
    "downloads_list",
    "download_answer",
    "downloads_clear_history",
    "download_remove",
    "download_delete_file",
    "download_open",
    "download_reveal",
    "permission_answer",
    "custom_list",
    "custom_read",
    "custom_write",
    "custom_import_file",
    "custom_import_url",
    "custom_duplicate",
    "custom_delete",
    "custom_set_file_enabled",
    "custom_open_folder",
    "custom_pick_preview",
    "custom_preview",
    "custom_reload",
    "spell_state",
    "spell_set_languages",
    "spell_install",
    "spell_remove",
    "spell_import",
    "spell_open_folder",
    "report_preview",
    "report_save",
    "report_open_github",
    "report_list",
    "report_pending",
    "report_keep",
    "report_delete",
    "report_clear",
    "report_open_folder",
    "update_info",
    "report_copied",
    "evo_instances",
    "evo_setup",
    "evo_connection",
    "evo_reconnect",
    "evo_qr",
    "evo_pair_phone",
    "evo_logout",
    "evo_chats",
    "evo_messages",
    "evo_set_open_chat",
    "evo_mark_read",
    "evo_typing",
    "evo_send_text",
    "evo_retry",
    "evo_send_file",
    "evo_react",
    "evo_edit",
    "evo_delete",
    "evo_media",
    "evo_save_media",
    "evo_load_older",
    "evo_avatar",
    "evo_refresh_contacts",
    "evo_info",
    "evo_open_passkey",
];

/// Commands exposed to https://web.whatsapp.com.
const BRIDGE_COMMANDS: &[&str] = &[
    "wa_title_changed",
    "wa_shortcut",
    "wa_theme_event",
    "wa_open_account_selector",
    "wa_profile_photo",
    "wa_notify",
    "wa_notification_close",
];

fn main() {
    generate_catalogs();
    let commands: &'static [&str] = UI_COMMANDS
        .iter()
        .chain(BRIDGE_COMMANDS)
        .copied()
        .collect::<Vec<_>>()
        .leak();
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(commands)),
    )
    .expect("failed to run tauri-build");
}

/// Embed `src/lib/i18n/catalogs/<locale>.json` as `CATALOGS: &[(locale, json)]`.
fn generate_catalogs() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/i18n/catalogs");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();

    let mut out = String::from("pub static CATALOGS: &[(&str, &str)] = &[\n");
    for path in files {
        let locale = path.file_stem().unwrap().to_string_lossy();
        let path = path.canonicalize().unwrap();
        println!("cargo:rerun-if-changed={}", path.display());
        writeln!(
            out,
            "    ({locale:?}, include_str!({:?})),",
            path.display().to_string()
        )
        .unwrap();
    }
    out.push_str("];\n");
    let target = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("catalogs.rs");
    std::fs::write(target, out).unwrap();
}
