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
