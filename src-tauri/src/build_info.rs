//! Build provenance, set by packaging workflows through environment variables
//! at compile time (ZapZap's generated `BuildInfo.py`).

use serde::Serialize;
use ts_rs::TS;

pub const CHANNEL: &str = match option_env!("FASTZAP_BUILD_CHANNEL") {
    Some(v) => v,
    None => "Unknown",
};
pub const PACKAGING: &str = match option_env!("FASTZAP_BUILD_PACKAGING") {
    Some(v) => v,
    None => "Source",
};
pub const PROVIDER: &str = match option_env!("FASTZAP_BUILD_PROVIDER") {
    Some(v) => v,
    None => "",
};
pub const REPOSITORY: &str = match option_env!("FASTZAP_BUILD_REPOSITORY") {
    Some(v) => v,
    None => "",
};

/// Official repository, used for releases and problem reports.
pub const OFFICIAL_REPOSITORY: &str = "quantforge-br/fastzap";

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub flatpak: bool,
    pub channel: String,
    pub packaging: String,
    pub provider: String,
    pub repository: String,
    pub desktop: String,
    pub session_type: String,
    pub webview_version: String,
}

pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        flatpak: crate::platform::is_flatpak(),
        channel: CHANNEL.into(),
        packaging: PACKAGING.into(),
        provider: PROVIDER.into(),
        repository: REPOSITORY.into(),
        desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        session_type: std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
        webview_version: tauri::webview_version().unwrap_or_default(),
    }
}
