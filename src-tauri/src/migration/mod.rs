//! One-shot import of an existing ZapZap installation.
//!
//! The import never modifies ZapZap's files. WhatsApp Web sessions (QtWebEngine
//! profiles) and `.bdic` dictionaries are not portable and are skipped.

pub mod qsettings;

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use ts_rs::TS;

use crate::accounts::{Account, AccountKind, AccountStore};
use crate::secrets::{PROXY_PASSWORD, Secret, SecretStore};
use crate::settings::*;
use qsettings::QSettingsIni;

/// ZapZap names the profile of account 1 `storage-whats`.
const LEGACY_FIRST_PROFILE: &str = "storage-whats";

#[derive(Debug, Clone, PartialEq)]
pub struct ZapZapSource {
    pub config_file: PathBuf,
    pub data_dir: PathBuf,
}

#[derive(Debug, Default, Clone, Serialize, TS)]
#[ts(export)]
pub struct ImportReport {
    pub source: String,
    pub settings_imported: bool,
    pub accounts_imported: u32,
    pub customization_files_copied: u32,
    pub warnings: Vec<String>,
}

/// Locate ZapZap data, preferring the most recently written config. Native
/// installs follow `XDG_CONFIG_HOME`/`XDG_DATA_HOME`; the Flatpak paths are fixed.
pub fn detect(home: &Path) -> Option<ZapZapSource> {
    let xdg = |var: &str, fallback: &str| {
        std::env::var_os(var)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(fallback))
    };
    detect_in(
        home,
        &xdg("XDG_CONFIG_HOME", ".config"),
        &xdg("XDG_DATA_HOME", ".local/share"),
    )
}

fn detect_in(home: &Path, config_home: &Path, data_home: &Path) -> Option<ZapZapSource> {
    let candidates = [
        ZapZapSource {
            config_file: config_home.join("ZapZap/ZapZap.conf"),
            data_dir: data_home.join("ZapZap"),
        },
        ZapZapSource {
            config_file: home.join(".var/app/com.rtosta.zapzap/config/ZapZap/ZapZap.conf"),
            data_dir: home.join(".var/app/com.rtosta.zapzap/data/ZapZap"),
        },
    ];
    candidates
        .into_iter()
        .filter(|c| c.config_file.is_file() || c.data_dir.join("db/zapzap.db").is_file())
        .max_by_key(|c| fs::metadata(&c.config_file).and_then(|m| m.modified()).ok())
}

/// Import `source` into the given stores. `customizations_dir` is FastZap's
/// customization root; existing files, accounts and secrets are never
/// overwritten, so an incomplete import can be retried. Preferences are only
/// applied when `include_settings` is set (the first attempt).
pub fn import(
    source: &ZapZapSource,
    settings: &mut Settings,
    accounts: &AccountStore,
    customizations_dir: &Path,
    secrets: &dyn SecretStore,
    include_settings: bool,
) -> ImportReport {
    let mut report = ImportReport {
        source: source.config_file.display().to_string(),
        ..Default::default()
    };

    let ini = match fs::read_to_string(&source.config_file) {
        Ok(text) => {
            let ini = QSettingsIni::parse(&text);
            if include_settings {
                apply_settings(&ini, settings);
                report.settings_imported = true;
            }
            if let Some(password) = ini.string("proxy/password").filter(|p| !p.is_empty())
                && let Err(e) = import_secret(secrets, PROXY_PASSWORD, password)
            {
                report
                    .warnings
                    .push(format!("proxy password not stored: {e}"));
            }
            ini
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => QSettingsIni::default(),
        Err(e) => {
            report.warnings.push(format!("settings not read: {e}"));
            QSettingsIni::default()
        }
    };
    match import_accounts(&source.data_dir.join("db/zapzap.db"), &ini, accounts) {
        Ok(n) => report.accounts_imported = n,
        Err(e) => report.warnings.push(format!("accounts not imported: {e}")),
    }

    match copy_tree(&source.data_dir.join("customizations"), customizations_dir) {
        Ok(n) => report.customization_files_copied = n,
        Err(e) => report
            .warnings
            .push(format!("customizations not copied: {e}")),
    }

    settings.migration.zapzap_imported_from = Some(report.source.clone());
    settings.normalize();
    report
}

/// Store a secret unless FastZap already has one under that key.
fn import_secret(
    secrets: &dyn SecretStore,
    key: &str,
    value: String,
) -> Result<(), crate::secrets::SecretError> {
    if secrets.get(key)?.is_none() {
        secrets.set(key, &Secret::new(value))?;
    }
    Ok(())
}

fn apply_settings(ini: &QSettingsIni, s: &mut Settings) {
    macro_rules! set_bool {
        ($key:expr, $field:expr) => {
            if let Some(v) = ini.bool($key) {
                $field = v;
            }
        };
    }
    macro_rules! set_string {
        ($key:expr, $field:expr) => {
            if let Some(v) = ini.string($key) {
                $field = v;
            }
        };
    }

    // Appearance (`system/` group in ZapZap).
    let a = &mut s.appearance;
    set_bool!("system/sidebar", a.sidebar);
    set_bool!("system/menubar", a.menubar);
    if let Some(v) = ini.int("system/scale") {
        a.scale = v.clamp(1, 1000) as u32;
    }
    set_bool!("system/tray_icon", a.tray_icon);
    set_bool!("system/notificationCounter", a.notification_counter);
    set_bool!("system/csr", a.csr);
    set_string!("system/csr_button_theme", a.csr_button_theme);
    set_bool!(
        "system/csr_show_minimize_button",
        a.csr_show_minimize_button
    );
    set_bool!(
        "system/csr_show_maximize_button",
        a.csr_show_maximize_button
    );
    if let Some(v) = ini.string("system/csr_buttons_direction") {
        a.csr_buttons_direction = if v.trim().eq_ignore_ascii_case("left") {
            ButtonsDirection::Left
        } else {
            ButtonsDirection::Right
        };
    }
    if let Some(v) = ini.string("system/theme") {
        a.theme = match v.as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::Auto,
        };
    }
    if let Some(v) = ini.string("system/tray_theme") {
        a.tray_theme = match v.as_str() {
            "symbolic_light" => TrayTheme::SymbolicLight,
            "symbolic_dark" => TrayTheme::SymbolicDark,
            _ => TrayTheme::Default,
        };
    }
    if let Some(v) = ini.int("system/grid_cols") {
        a.grid_cols = v.clamp(1, 6) as u32;
    }
    let p = &mut a.floating_panel;
    set_bool!("system/floating_monitoring_panel_enabled", p.enabled);
    if let Some(v) = ini.string("system/floating_monitoring_panel_mode") {
        p.mode = match v.as_str() {
            "when_hidden" | "hidden" => FloatingPanelMode::WhenHidden,
            "on_demand" | "manual" => FloatingPanelMode::OnDemand,
            _ => FloatingPanelMode::Always,
        };
    }
    set_bool!("system/floating_monitoring_panel_on_top", p.on_top);
    set_bool!(
        "system/floating_monitoring_panel_remember_position",
        p.remember_position
    );
    set_bool!(
        "system/floating_monitoring_panel_lock_position",
        p.lock_position
    );
    if let Some(pos) = ini.point("system/floating_monitoring_panel_position") {
        p.position = Some(pos);
    }
    set_bool!(
        "system/integrated_account_selector_enabled",
        a.integrated_account_selector_enabled
    );
    set_bool!(
        "system/resizable_chat_list_enabled",
        a.resizable_chat_list_enabled
    );

    // System. `display_backend` wins; otherwise migrate the legacy `wayland` switch.
    let sys = &mut s.system;
    sys.display_backend = match ini.string("system/display_backend").as_deref() {
        Some("wayland") => DisplayBackend::Wayland,
        Some("xcb") => DisplayBackend::Xcb,
        Some(_) => DisplayBackend::Auto,
        None if ini.bool("system/wayland") == Some(true) => DisplayBackend::Wayland,
        None => sys.display_backend,
    };
    set_bool!("system/confirm_on_close", sys.confirm_on_close);
    set_bool!("system/quit_in_close", sys.quit_on_close);
    set_bool!("system/start_background", sys.start_background);
    set_bool!("system/start_system", sys.start_system);
    set_bool!("system/DontUseNativeDialog", sys.dont_use_native_dialog);
    set_bool!("system/audio_muted", sys.audio_muted);
    set_string!("system/interface_language", sys.interface_language);
    set_bool!("website/open_page", sys.open_website_on_first_run);
    if let Some(v) = ini.int("users/size") {
        sys.max_accounts = v.max(1) as u32;
    }
    set_string!("QTWEBENGINE_CHROMIUM_FLAGS", sys.extra_engine_flags);

    // Notifications.
    let n = &mut s.notifications;
    set_bool!("notification/app", n.enabled);
    set_bool!("notification/show_photo", n.show_photo);
    set_bool!("notification/show_name", n.show_name);
    set_bool!("notification/show_msg", n.show_message);
    set_bool!("notification/sound", n.sound);
    set_bool!("notification/donation_message", n.donation_message_hidden);

    // Downloads, including the legacy `auto_open_media` split.
    let d = &mut s.downloads;
    set_bool!(
        "downloads/remember_last_save_as_directory",
        d.remember_last_save_as_directory
    );
    if let Some(v) = ini.string("downloads/behavior") {
        d.behavior = match v.as_str() {
            "automatic" => DownloadBehavior::Automatic,
            "ask_every_time" => DownloadBehavior::AskEveryTime,
            _ => DownloadBehavior::Dialog,
        };
    }
    if let Some(legacy) = ini.bool("downloads/auto_open_media") {
        d.auto_open_pdf = legacy;
        d.auto_open_images = legacy;
    }
    set_bool!("downloads/auto_open_pdf", d.auto_open_pdf);
    set_bool!("downloads/auto_open_images", d.auto_open_images);
    if let Some(v) = ini.string("downloads/pdf_click_behavior") {
        d.pdf_click_behavior = if v == "download" {
            PdfClickBehavior::Download
        } else {
            PdfClickBehavior::Preview
        };
    }
    if let Some(v) = ini.string("downloads/whatsapp_multiple_download_permission") {
        d.multiple_download_permission = match v.as_str() {
            "allow" => TriState::Allow,
            "block" => TriState::Block,
            _ => TriState::Ask,
        };
    }
    set_string!("system/download_path", d.download_path);
    if let Some(v) = ini.list("system/recent_downloads") {
        d.recent = v;
    }

    // Performance (preserved even when it has no effect on system webviews).
    let pf = &mut s.performance;
    set_string!("performance/cache_type", pf.cache_type);
    set_string!("performance/cache_size_max", pf.cache_size_max);
    if let Some(v) = ini.int("performance/js_memory_limit_index") {
        pf.js_memory_limit_index = v as i32;
    }
    set_string!("performance/js_memory_limit_mb", pf.js_memory_limit_mb);
    for (key, field) in [
        ("persistent_cookies", &mut pf.persistent_cookies),
        ("in_process_gpu", &mut pf.in_process_gpu),
        ("disable_gpu", &mut pf.disable_gpu),
        ("auto_gpu_workaround", &mut pf.auto_gpu_workaround),
        ("disable_gpu_vsync", &mut pf.disable_gpu_vsync),
        ("software_rendering", &mut pf.software_rendering),
        (
            "disable_gpu_memory_buffer_video_frames",
            &mut pf.disable_gpu_memory_buffer_video_frames,
        ),
        ("disable_zero_copy", &mut pf.disable_zero_copy),
        ("software_video_decoding", &mut pf.software_video_decoding),
        ("force_gbm", &mut pf.force_gbm),
        ("disable_accessibility", &mut pf.disable_accessibility),
        ("single_process", &mut pf.single_process),
        ("process_per_site", &mut pf.process_per_site),
        (
            "js_predictable_gc_schedule",
            &mut pf.js_predictable_gc_schedule,
        ),
    ] {
        if let Some(v) = ini.bool(&format!("performance/{key}")) {
            *field = v;
        }
    }
    for (key, field) in [
        ("scroll_animator", &mut pf.scroll_animator),
        ("background_throttling", &mut pf.background_throttling),
        ("disable_animations", &mut pf.disable_animations),
        ("disable_pinch", &mut pf.disable_pinch),
        (
            "ctrl_arrow_visual_navigation_fix",
            &mut pf.ctrl_arrow_visual_navigation_fix,
        ),
    ] {
        if let Some(v) = ini.bool(&format!("web/{key}")) {
            *field = v;
        }
    }

    set_bool!("privacy/webrtc_shield", s.privacy.webrtc_shield);
    set_bool!("privacy/strict_proxy", s.privacy.strict_proxy);
    set_bool!("reporting/crash_prompts", s.reporting.crash_prompts);
    set_bool!(
        "reporting/crash_prompts_explained",
        s.reporting.crash_prompts_explained
    );

    // Spellcheck: the list wins over the legacy single language.
    let sp = &mut s.spellcheck;
    set_bool!("system/spellCheckers", sp.enabled);
    match ini.list("system/spellCheckLanguages") {
        Some(list) if !list.is_empty() => sp.languages = list,
        _ => {
            if let Some(single) = ini
                .string("system/spellCheckLanguage")
                .filter(|v| !v.is_empty())
            {
                sp.languages = vec![single];
            }
        }
    }
    if let Some(v) = ini.list("system/recentSpellCheckLanguages") {
        sp.recent_languages = v;
    }
    if let Some(v) = ini.string("system/spellCheckProvisionedLocale") {
        sp.provisioned_locale = Some(v);
    }

    // Proxy (password goes to the keyring in `import`).
    let px = &mut s.proxy;
    set_bool!("proxy/proxyEnable", px.enabled);
    if let Some(v) = ini.string("proxy/proxyType") {
        px.kind = match v.as_str() {
            "DefaultProxy" => ProxyKind::DefaultProxy,
            "Socks5Proxy" => ProxyKind::Socks5Proxy,
            "HttpProxy" => ProxyKind::HttpProxy,
            "HttpCachingProxy" => ProxyKind::HttpCachingProxy,
            "FtpCachingProxy" => ProxyKind::FtpCachingProxy,
            _ => ProxyKind::NoProxy,
        };
    }
    set_string!("proxy/hostName", px.host);
    if let Some(v) = ini.int("proxy/port") {
        px.port = v.clamp(0, 65535) as u32;
    }
    set_string!("proxy/user", px.user);

    let g = &mut s.permissions.auto_grant;
    for (key, field) in [
        ("microphone", &mut g.microphone),
        ("camera", &mut g.camera),
        ("camera_microphone", &mut g.camera_microphone),
        ("location", &mut g.location),
        ("screen_contents", &mut g.screen_contents),
        ("screen_contents_audio", &mut g.screen_contents_audio),
        ("mouse_lock", &mut g.mouse_lock),
    ] {
        if let Some(v) = ini.bool(&format!("permissions/auto_grant/{key}")) {
            *field = v;
        }
    }

    // Customizations: global scope and every `custom/accounts/<id>` scope.
    let c = &mut s.customizations;
    read_toggle(ini, "custom/global/css", &mut c.global.css);
    read_toggle(ini, "custom/global/js", &mut c.global.js);
    let account_ids: std::collections::BTreeSet<String> = ini
        .keys()
        .filter_map(|k| k.strip_prefix("custom/accounts/"))
        .filter_map(|rest| rest.split('/').next())
        .map(str::to_owned)
        .collect();
    for id in account_ids {
        let entry = c.accounts.entry(id.clone()).or_default();
        if let Some(v) = ini.bool(&format!("custom/accounts/{id}/inherit")) {
            entry.inherit = v;
        }
        read_toggle(ini, &format!("custom/accounts/{id}/css"), &mut entry.css);
        read_toggle(ini, &format!("custom/accounts/{id}/js"), &mut entry.js);
    }

    if let Some(v) = ini.bool("onboarding/initial_setup_completed") {
        s.onboarding.completed = v;
    }
}

fn read_toggle(ini: &QSettingsIni, prefix: &str, toggle: &mut AssetToggle) {
    if let Some(v) = ini.bool(&format!("{prefix}/enabled")) {
        toggle.enabled = v;
    }
    if let Some(v) = ini.list(&format!("{prefix}/disabled_files")) {
        toggle.disabled_files = v;
    }
    if let Some(v) = ini.map(&format!("{prefix}/preview_map")) {
        toggle.preview_map = v
            .into_iter()
            .filter(|(k, v)| !k.is_empty() && !v.is_empty())
            .collect();
    }
}

fn profile_key(id: i64) -> String {
    if id == 1 {
        LEGACY_FIRST_PROFILE.to_owned()
    } else {
        id.to_string()
    }
}

fn import_accounts(
    db: &Path,
    ini: &QSettingsIni,
    accounts: &AccountStore,
) -> Result<u32, Box<dyn std::error::Error>> {
    if !db.is_file() {
        return Ok(0);
    }
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut stmt =
        conn.prepare("SELECT id, name, icon, enable, zoomFactor FROM users ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<f64>>(4)?,
        ))
    })?;
    let mut count = 0;
    for (position, row) in rows.enumerate() {
        let (id, name, icon, enable, zoom) = row?;
        let key = profile_key(id);
        let account = Account {
            id,
            name: name.unwrap_or_default(),
            icon: icon.filter(|i| !i.is_empty()),
            enabled: enable.unwrap_or(1) != 0,
            zoom: zoom.unwrap_or(1.0),
            kind: AccountKind::WhatsappWeb,
            position: position as i64,
            user_agent: ini
                .string(&format!("{key}/user_agent"))
                .unwrap_or_else(|| "Default".into()),
            notifications: ini.bool(&format!("{key}/notification")).unwrap_or(true),
            evolution: None,
        };
        if accounts.insert_imported(&account)? {
            count += 1;
        }
    }
    Ok(count)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<u32> {
    if !from.is_dir() {
        return Ok(0);
    }
    let mut copied = 0;
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copied += copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() && !target.exists() {
            fs::copy(entry.path(), &target)?;
            copied += 1;
        }
    }
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemoryStore;

    const CONF: &str = r#"
[General]
QTWEBENGINE_CHROMIUM_FLAGS=--enable-foo

[custom]
global\css\enabled=true
global\css\disabled_files=a.css, b.css
accounts\2\inherit=false
accounts\2\js\enabled=true

[downloads]
auto_open_media=true
auto_open_pdf=false
whatsapp_multiple_download_permission=allow

[notification]
show_msg=false
donation_message=true

[permissions]
auto_grant\camera=true

[proxy]
proxyEnable=true
proxyType=Socks5Proxy
hostName=127.0.0.1
port=1080
user=bob
password=s3cret

[storage-whats]
user_agent=Linux Firefox
notification=false

[system]
theme=dark
tray_theme=symbolic_light
quit_in_close=true
wayland=true
spellCheckLanguage=pt_BR
spellCheckers=false
scale=125

[users]
size=8

[web]
disable_pinch=true
"#;

    fn make_source(dir: &Path) -> ZapZapSource {
        let config_file = dir.join("config/ZapZap.conf");
        let data_dir = dir.join("data");
        fs::create_dir_all(config_file.parent().unwrap()).unwrap();
        fs::write(&config_file, CONF).unwrap();
        fs::create_dir_all(data_dir.join("db")).unwrap();
        let conn = Connection::open(data_dir.join("db/zapzap.db")).unwrap();
        conn.execute_batch(
            r#"CREATE TABLE "users" ("id" INTEGER PRIMARY KEY AUTOINCREMENT, "name" TEXT NOT NULL,
               "icon" TEXT, "enable" INTEGER DEFAULT 1, "zoomFactor" REAL DEFAULT 1.0);
               INSERT INTO users VALUES (1, '', '<svg/>', 1, 1.0);
               INSERT INTO users VALUES (2, 'Work', NULL, 0, 1.5);"#,
        )
        .unwrap();
        fs::create_dir_all(data_dir.join("customizations/global/css")).unwrap();
        fs::write(data_dir.join("customizations/global/css/a.css"), "body{}").unwrap();
        ZapZapSource {
            config_file,
            data_dir,
        }
    }

    #[test]
    fn imports_settings_accounts_and_files() {
        let dir = tempfile::tempdir().unwrap();
        let source = make_source(dir.path());
        let accounts = AccountStore::in_memory().unwrap();
        let secrets = MemoryStore::default();
        let mut settings = Settings::default();
        let custom = dir.path().join("fastzap/customizations");

        let report = import(&source, &mut settings, &accounts, &custom, &secrets, true);

        assert!(report.settings_imported);
        assert_eq!(report.accounts_imported, 2);
        assert_eq!(report.customization_files_copied, 1);
        assert!(custom.join("global/css/a.css").is_file());

        assert_eq!(settings.appearance.theme, ThemeMode::Dark);
        assert_eq!(settings.appearance.tray_theme, TrayTheme::SymbolicLight);
        assert_eq!(settings.appearance.scale, 125);
        assert!(settings.system.quit_on_close);
        assert_eq!(settings.system.display_backend, DisplayBackend::Wayland);
        assert_eq!(settings.system.max_accounts, 8);
        assert_eq!(settings.system.extra_engine_flags, "--enable-foo");
        assert!(!settings.notifications.show_message);
        assert!(settings.notifications.donation_message_hidden);
        // Explicit key wins over the legacy `auto_open_media`.
        assert!(!settings.downloads.auto_open_pdf);
        assert!(settings.downloads.auto_open_images);
        assert_eq!(
            settings.downloads.multiple_download_permission,
            TriState::Allow
        );
        assert!(settings.performance.disable_pinch);
        assert!(!settings.spellcheck.enabled);
        assert_eq!(settings.spellcheck.languages, ["pt_BR"]);
        assert!(settings.permissions.auto_grant.camera);
        assert_eq!(settings.proxy.kind, ProxyKind::Socks5Proxy);
        assert_eq!(settings.proxy.port, 1080);
        assert_eq!(
            secrets.get(PROXY_PASSWORD).unwrap().unwrap().expose(),
            "s3cret"
        );
        assert!(settings.customizations.global.css.enabled);
        assert_eq!(
            settings.customizations.global.css.disabled_files,
            ["a.css", "b.css"]
        );
        let acc2 = &settings.customizations.accounts["2"];
        assert!(!acc2.inherit && acc2.js.enabled);
        assert!(report.warnings.is_empty());
        assert!(settings.migration.zapzap_imported_from.is_some());

        let list = accounts.list().unwrap();
        assert_eq!(list[0].id, 1);
        assert_eq!(list[0].user_agent, "Linux Firefox");
        assert!(!list[0].notifications);
        assert_eq!(list[0].icon.as_deref(), Some("<svg/>"));
        assert_eq!(list[1].name, "Work");
        assert!(!list[1].enabled);
        assert_eq!(list[1].zoom, 1.5);
        assert!(list[1].notifications);
    }

    #[test]
    fn existing_customizations_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let source = make_source(dir.path());
        let custom = dir.path().join("fastzap/customizations");
        fs::create_dir_all(custom.join("global/css")).unwrap();
        fs::write(custom.join("global/css/a.css"), "mine").unwrap();
        let mut settings = Settings::default();
        import(
            &source,
            &mut settings,
            &AccountStore::in_memory().unwrap(),
            &custom,
            &MemoryStore::default(),
            true,
        );
        assert_eq!(
            fs::read_to_string(custom.join("global/css/a.css")).unwrap(),
            "mine"
        );
    }

    #[test]
    fn detect_prefers_existing_installation() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let (config_home, data_home) = (home.join(".config"), home.join(".local/share"));
        assert!(detect_in(home, &config_home, &data_home).is_none());
        let flatpak = home.join(".var/app/com.rtosta.zapzap/config/ZapZap/ZapZap.conf");
        fs::create_dir_all(flatpak.parent().unwrap()).unwrap();
        fs::write(&flatpak, "[system]\ntheme=dark\n").unwrap();
        assert_eq!(
            detect_in(home, &config_home, &data_home)
                .unwrap()
                .config_file,
            flatpak
        );
    }

    #[test]
    fn detect_follows_xdg_directories() {
        let dir = tempfile::tempdir().unwrap();
        let config_home = dir.path().join("cfg");
        let native = config_home.join("ZapZap/ZapZap.conf");
        fs::create_dir_all(native.parent().unwrap()).unwrap();
        fs::write(&native, "").unwrap();
        let found = detect_in(dir.path(), &config_home, &dir.path().join("data")).unwrap();
        assert_eq!(found.config_file, native);
        assert_eq!(found.data_dir, dir.path().join("data/ZapZap"));
    }

    #[test]
    fn existing_proxy_secret_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let source = make_source(dir.path());
        let secrets = MemoryStore::default();
        secrets.set(PROXY_PASSWORD, &Secret::new("mine")).unwrap();
        import(
            &source,
            &mut Settings::default(),
            &AccountStore::in_memory().unwrap(),
            &dir.path().join("custom"),
            &secrets,
            true,
        );
        assert_eq!(
            secrets.get(PROXY_PASSWORD).unwrap().unwrap().expose(),
            "mine"
        );
    }

    #[test]
    fn retry_fills_missing_accounts_without_touching_settings() {
        let dir = tempfile::tempdir().unwrap();
        let source = make_source(dir.path());
        let accounts = AccountStore::in_memory().unwrap();
        let mut existing = Account {
            id: 1,
            name: "Edited".into(),
            icon: None,
            enabled: true,
            zoom: 1.0,
            kind: AccountKind::WhatsappWeb,
            position: 0,
            user_agent: "Default".into(),
            notifications: true,
            evolution: None,
        };
        accounts.insert_imported(&existing).unwrap();
        let mut settings = Settings::default();
        let report = import(
            &source,
            &mut settings,
            &accounts,
            &dir.path().join("custom"),
            &MemoryStore::default(),
            false,
        );
        assert!(!report.settings_imported);
        assert_eq!(report.accounts_imported, 1);
        assert_eq!(settings.appearance.theme, ThemeMode::Auto);
        existing.name = accounts.get(1).unwrap().name;
        assert_eq!(existing.name, "Edited");
        assert_eq!(accounts.count().unwrap(), 2);
    }
}
