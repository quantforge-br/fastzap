//! Typed application preferences.
//!
//! Names and defaults mirror ZapZap's `zapzap/core/config/settings/` domains so
//! that imported values keep their meaning. Every struct uses `#[serde(default)]`
//! so partial or older files load without losing unknown-but-valid fields.

pub mod store;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const SETTINGS_VERSION: u32 = 1;
pub const DEFAULT_MAX_ACCOUNTS: u32 = 5;
pub const MAX_SPELLCHECK_LANGUAGES: usize = 10;
pub const MAX_RECENT_SPELLCHECK_LANGUAGES: usize = 4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct Settings {
    pub version: u32,
    pub appearance: AppearanceSettings,
    pub system: SystemSettings,
    pub notifications: NotificationSettings,
    pub downloads: DownloadSettings,
    pub performance: PerformanceSettings,
    pub privacy: PrivacySettings,
    pub reporting: ReportingSettings,
    pub spellcheck: SpellcheckSettings,
    pub proxy: ProxySettings,
    pub permissions: PermissionSettings,
    pub customizations: CustomizationSettings,
    pub onboarding: OnboardingSettings,
    pub migration: MigrationState,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            appearance: Default::default(),
            system: Default::default(),
            notifications: Default::default(),
            downloads: Default::default(),
            performance: Default::default(),
            privacy: Default::default(),
            reporting: Default::default(),
            spellcheck: Default::default(),
            proxy: Default::default(),
            permissions: Default::default(),
            customizations: Default::default(),
            onboarding: Default::default(),
            migration: Default::default(),
        }
    }
}

impl Settings {
    /// Clamp and normalize values that may come from older files, imports or the UI.
    pub fn normalize(&mut self) {
        self.version = SETTINGS_VERSION;
        let a = &mut self.appearance;
        a.scale = a.scale.clamp(1, 1000);
        a.grid_cols = a.grid_cols.clamp(1, 6);
        let s = &mut self.system;
        s.max_accounts = s.max_accounts.max(1);
        let sp = &mut self.spellcheck;
        sp.languages.dedup();
        sp.languages.truncate(MAX_SPELLCHECK_LANGUAGES);
        sp.recent_languages
            .truncate(MAX_RECENT_SPELLCHECK_LANGUAGES);
        self.downloads.recent.truncate(crate::downloads::MAX_RECENT);
        if self.proxy.port > 65535 {
            self.proxy.port = 0;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Auto,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TrayTheme {
    Default,
    SymbolicLight,
    SymbolicDark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ButtonsDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FloatingPanelMode {
    Always,
    WhenHidden,
    OnDemand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct AppearanceSettings {
    pub sidebar: bool,
    pub menubar: bool,
    /// Interface scale in percent (`system/scale`).
    pub scale: u32,
    pub tray_icon: bool,
    pub notification_counter: bool,
    /// Off: no title bar at all, native or drawn by FastZap (tiling WMs).
    pub title_bar: bool,
    pub csr: bool,
    pub csr_button_theme: String,
    pub csr_show_minimize_button: bool,
    pub csr_show_maximize_button: bool,
    pub csr_buttons_direction: ButtonsDirection,
    pub theme: ThemeMode,
    pub tray_theme: TrayTheme,
    pub grid_cols: u32,
    pub floating_panel: FloatingPanelSettings,
    pub integrated_account_selector_enabled: bool,
    pub resizable_chat_list_enabled: bool,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            sidebar: true,
            menubar: true,
            scale: 100,
            tray_icon: true,
            notification_counter: true,
            title_bar: true,
            csr: false,
            csr_button_theme: "default".into(),
            csr_show_minimize_button: true,
            csr_show_maximize_button: true,
            csr_buttons_direction: ButtonsDirection::Right,
            theme: ThemeMode::Auto,
            tray_theme: TrayTheme::Default,
            grid_cols: 2,
            floating_panel: Default::default(),
            integrated_account_selector_enabled: false,
            resizable_chat_list_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct FloatingPanelSettings {
    pub enabled: bool,
    pub mode: FloatingPanelMode,
    pub on_top: bool,
    pub remember_position: bool,
    pub lock_position: bool,
    pub position: Option<(i32, i32)>,
}

impl Default for FloatingPanelSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: FloatingPanelMode::Always,
            on_top: false,
            remember_position: true,
            lock_position: false,
            position: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DisplayBackend {
    Auto,
    Wayland,
    Xcb,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct SystemSettings {
    pub display_backend: DisplayBackend,
    pub confirm_on_close: bool,
    /// `true` quits on close; `false` keeps running in the tray.
    pub quit_on_close: bool,
    pub start_background: bool,
    pub start_system: bool,
    pub dont_use_native_dialog: bool,
    pub audio_muted: bool,
    /// `"system"` or a locale code such as `pt_BR`.
    pub interface_language: String,
    pub open_website_on_first_run: bool,
    pub max_accounts: u32,
    /// Extra engine flags kept from ZapZap's `QTWEBENGINE_CHROMIUM_FLAGS`.
    pub extra_engine_flags: String,
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            display_backend: DisplayBackend::Auto,
            confirm_on_close: false,
            quit_on_close: false,
            start_background: false,
            start_system: false,
            dont_use_native_dialog: false,
            audio_muted: false,
            interface_language: "system".into(),
            open_website_on_first_run: true,
            max_accounts: DEFAULT_MAX_ACCOUNTS,
            extra_engine_flags: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct NotificationSettings {
    pub enabled: bool,
    pub show_photo: bool,
    pub show_name: bool,
    pub show_message: bool,
    pub sound: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            show_photo: true,
            show_name: true,
            show_message: true,
            sound: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DownloadBehavior {
    Dialog,
    Automatic,
    AskEveryTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PdfClickBehavior {
    Preview,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TriState {
    Ask,
    Allow,
    Block,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct DownloadSettings {
    pub remember_last_save_as_directory: bool,
    pub behavior: DownloadBehavior,
    pub auto_open_pdf: bool,
    pub auto_open_images: bool,
    pub pdf_click_behavior: PdfClickBehavior,
    pub multiple_download_permission: TriState,
    /// Empty means the system Downloads directory.
    pub download_path: String,
    pub recent: Vec<String>,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            remember_last_save_as_directory: true,
            behavior: DownloadBehavior::Dialog,
            auto_open_pdf: false,
            auto_open_images: false,
            pdf_click_behavior: PdfClickBehavior::Preview,
            multiple_download_permission: TriState::Ask,
            download_path: String::new(),
            recent: Vec::new(),
        }
    }
}

/// ZapZap performance keys. Only a subset maps to the system webviews; the
/// rest are preserved so a round trip never loses the user's choices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct PerformanceSettings {
    pub cache_type: String,
    pub cache_size_max: String,
    pub js_memory_limit_index: i32,
    pub js_memory_limit_mb: String,
    pub persistent_cookies: bool,
    pub in_process_gpu: bool,
    pub disable_gpu: bool,
    pub auto_gpu_workaround: bool,
    pub disable_gpu_vsync: bool,
    pub software_rendering: bool,
    pub disable_gpu_memory_buffer_video_frames: bool,
    pub disable_zero_copy: bool,
    pub software_video_decoding: bool,
    pub force_gbm: bool,
    pub disable_accessibility: bool,
    pub single_process: bool,
    pub process_per_site: bool,
    pub js_predictable_gc_schedule: bool,
    pub scroll_animator: bool,
    pub background_throttling: bool,
    pub disable_animations: bool,
    pub disable_pinch: bool,
    pub ctrl_arrow_visual_navigation_fix: bool,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            cache_type: "DiskHttpCache".into(),
            cache_size_max: "0".into(),
            js_memory_limit_index: 0,
            js_memory_limit_mb: "0".into(),
            persistent_cookies: true,
            in_process_gpu: false,
            disable_gpu: false,
            auto_gpu_workaround: true,
            disable_gpu_vsync: false,
            software_rendering: false,
            disable_gpu_memory_buffer_video_frames: false,
            disable_zero_copy: false,
            software_video_decoding: false,
            force_gbm: false,
            disable_accessibility: false,
            single_process: false,
            process_per_site: true,
            js_predictable_gc_schedule: false,
            scroll_animator: false,
            background_throttling: true,
            disable_animations: false,
            disable_pinch: false,
            ctrl_arrow_visual_navigation_fix: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct PrivacySettings {
    pub webrtc_shield: bool,
    pub strict_proxy: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ReportingSettings {
    pub crash_prompts: bool,
    pub crash_prompts_explained: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct SpellcheckSettings {
    pub enabled: bool,
    pub languages: Vec<String>,
    pub recent_languages: Vec<String>,
    pub provisioned_locale: Option<String>,
}

impl Default for SpellcheckSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            languages: Vec::new(),
            recent_languages: Vec::new(),
            provisioned_locale: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ProxyKind {
    NoProxy,
    DefaultProxy,
    Socks5Proxy,
    HttpProxy,
    HttpCachingProxy,
    FtpCachingProxy,
}

/// Global proxy. The password lives in the system keyring (`secrets::PROXY_PASSWORD`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ProxySettings {
    pub enabled: bool,
    pub kind: ProxyKind,
    pub host: String,
    pub port: u32,
    pub user: String,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: ProxyKind::NoProxy,
            host: String::new(),
            port: 0,
            user: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct PermissionSettings {
    pub auto_grant: AutoGrant,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct AutoGrant {
    pub microphone: bool,
    pub camera: bool,
    pub camera_microphone: bool,
    pub location: bool,
    pub screen_contents: bool,
    pub screen_contents_audio: bool,
    pub mouse_lock: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct CustomizationSettings {
    pub global: CustomizationScope,
    /// Keyed by account id.
    pub accounts: BTreeMap<String, AccountCustomization>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct CustomizationScope {
    pub css: AssetToggle,
    pub js: AssetToggle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct AccountCustomization {
    /// When `true` the account only receives the global files.
    pub inherit: bool,
    pub css: AssetToggle,
    pub js: AssetToggle,
}

impl Default for AccountCustomization {
    fn default() -> Self {
        Self {
            inherit: true,
            css: Default::default(),
            js: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct AssetToggle {
    pub enabled: bool,
    pub disabled_files: Vec<String>,
    /// CSS file name → preview image file name.
    pub preview_map: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct OnboardingSettings {
    pub completed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct MigrationState {
    /// The ZapZap import is finished (completed, absent or out of retries).
    pub zapzap_checked: bool,
    /// Set once an import started; later attempts only fill in missing data.
    pub zapzap_imported_from: Option<String>,
    pub zapzap_attempts: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_zapzap() {
        let s = Settings::default();
        assert!(s.appearance.sidebar && s.appearance.tray_icon);
        assert_eq!(s.appearance.scale, 100);
        assert_eq!(s.appearance.grid_cols, 2);
        assert_eq!(s.appearance.theme, ThemeMode::Auto);
        assert!(!s.system.quit_on_close);
        assert_eq!(s.system.max_accounts, 5);
        assert!(s.notifications.enabled && s.notifications.sound);
        assert_eq!(s.downloads.behavior, DownloadBehavior::Dialog);
        assert_eq!(s.downloads.multiple_download_permission, TriState::Ask);
        assert!(s.performance.persistent_cookies && s.performance.process_per_site);
        assert!(s.performance.ctrl_arrow_visual_navigation_fix);
        assert!(s.spellcheck.enabled);
        assert!(!s.permissions.auto_grant.camera);
        assert!(AccountCustomization::default().inherit);
    }

    #[test]
    fn partial_json_keeps_defaults() {
        let s: Settings =
            serde_json::from_str(r#"{"appearance":{"theme":"dark"},"unknown":1}"#).unwrap();
        assert_eq!(s.appearance.theme, ThemeMode::Dark);
        assert!(s.appearance.sidebar);
        assert_eq!(s.system.max_accounts, 5);
    }

    #[test]
    fn normalize_clamps_values() {
        let mut s = Settings::default();
        s.appearance.scale = 0;
        s.appearance.grid_cols = 99;
        s.spellcheck.languages = (0..20).map(|i| format!("l{i}")).collect();
        s.normalize();
        assert_eq!(s.appearance.scale, 1);
        assert_eq!(s.appearance.grid_cols, 6);
        assert_eq!(s.spellcheck.languages.len(), MAX_SPELLCHECK_LANGUAGES);
    }
}
