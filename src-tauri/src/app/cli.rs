//! Command-line options (ZapZap's `startup_options.py`), parsed like
//! `argparse.parse_known_args`: unknown arguments are kept as links.

use crate::settings::{DisplayBackend, Settings};

pub const RESTART_MESSAGE: &str = "fastzap://restart";

#[derive(Debug, Default, PartialEq)]
pub struct StartupOptions {
    /// `--setSettings KEY VALUE`, with ZapZap's `group/key` names.
    pub set_settings: Vec<(String, String)>,
    /// `--wayland`: force the Wayland GDK backend.
    pub wayland: bool,
    /// `--hideStart`: start in the tray.
    pub hide_start: bool,
    /// Remaining arguments, e.g. `whatsapp:` links from the desktop file.
    pub links: Vec<String>,
}

/// Parse `args` without the program name.
pub fn parse<I, S>(args: I) -> StartupOptions
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut options = StartupOptions::default();
    let mut args = args.into_iter().map(Into::into);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--setSettings" => {
                if let (Some(key), Some(value)) = (args.next(), args.next()) {
                    options.set_settings.push((key, value));
                }
            }
            "--wayland" => options.wayland = true,
            "--hideStart" => options.hide_start = true,
            _ if arg.starts_with('-') => {}
            _ => options.links.push(arg),
        }
    }
    options
}

/// Apply `--setSettings` pairs, returning how many changed a setting.
pub fn apply_set_settings(options: &StartupOptions, settings: &mut Settings) -> usize {
    let mut applied = 0;
    for (key, value) in &options.set_settings {
        if crate::migration::apply_zapzap_setting(key, value, settings) {
            log::info!("setting {key} from the command line");
            applied += 1;
        } else {
            log::warn!("setting {key} unchanged: unknown key, invalid or same value");
        }
    }
    applied
}

/// GDK backend to force before GTK starts: `--wayland`, then an explicit
/// `GDK_BACKEND`, then the `display_backend` preference.
pub fn gdk_backend(
    options: &StartupOptions,
    env: Option<&str>,
    preference: DisplayBackend,
) -> Option<&'static str> {
    if options.wayland {
        return Some("wayland");
    }
    if env.is_some_and(|v| !v.is_empty()) {
        return None;
    }
    match preference {
        DisplayBackend::Auto => None,
        DisplayBackend::Wayland => Some("wayland"),
        DisplayBackend::Xcb => Some("x11"),
    }
}

/// Read the stored preferences before Tauri starts, without repairing or
/// rewriting the file (the settings store does that later).
pub fn early_settings() -> Settings {
    dirs::config_dir()
        .map(|dir| dir.join(crate::APP_ID).join("settings.json"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_options_and_links_are_split() {
        let options = parse([
            "--hideStart",
            "--setSettings",
            "system/start_background",
            "true",
            "--unknown",
            "whatsapp://send?phone=1",
            "--wayland",
        ]);
        assert!(options.hide_start && options.wayland);
        assert_eq!(
            options.set_settings,
            vec![("system/start_background".into(), "true".into())]
        );
        assert_eq!(options.links, vec!["whatsapp://send?phone=1".to_owned()]);
    }

    #[test]
    fn incomplete_set_settings_is_ignored() {
        assert!(parse(["--setSettings", "a"]).set_settings.is_empty());
    }

    #[test]
    fn set_settings_uses_zapzap_keys() {
        let options = parse(["--setSettings", "system/start_background", "true"]);
        let mut settings = Settings::default();
        assert_eq!(apply_set_settings(&options, &mut settings), 1);
        assert!(settings.system.start_background);
        let options = parse(["--setSettings", "nope/key", "1"]);
        assert_eq!(apply_set_settings(&options, &mut settings), 0);
    }

    #[test]
    fn wayland_flag_beats_environment_and_preference() {
        let flag = parse(["--wayland"]);
        let none = StartupOptions::default();
        assert_eq!(
            gdk_backend(&flag, Some("x11"), DisplayBackend::Xcb),
            Some("wayland")
        );
        assert_eq!(
            gdk_backend(&none, Some("x11"), DisplayBackend::Wayland),
            None
        );
        assert_eq!(gdk_backend(&none, None, DisplayBackend::Xcb), Some("x11"));
        assert_eq!(gdk_backend(&none, Some(""), DisplayBackend::Auto), None);
    }
}
