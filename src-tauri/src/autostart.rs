//! Start with the system (ZapZap's `autostart_manager.py`): the XDG
//! Background portal in Flatpak, an XDG autostart entry in other Linux
//! installs and `tauri-plugin-autostart` on Windows (Run key) and macOS
//! (LaunchAgent). Every entry starts FastZap with `--hideStart`.

use tauri::{AppHandle, Manager, Runtime};

pub const ARGS: &[&str] = &["--hideStart"];

pub async fn apply<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        if crate::platform::is_flatpak() {
            request_background(enabled).await
        } else {
            xdg::apply(app, enabled)
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        use tauri_plugin_autostart::ManagerExt;
        let launcher = app.autolaunch();
        if launcher.is_enabled()? != enabled {
            if enabled {
                launcher.enable()?;
            } else {
                launcher.disable()?;
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
async fn request_background(enabled: bool) -> anyhow::Result<()> {
    use ashpd::desktop::background::Background;
    let command: Vec<&str> = std::iter::once("fastzap")
        .chain(ARGS.iter().copied())
        .collect();
    Background::request()
        .reason("FastZap autostart")
        .auto_start(enabled)
        .command(command)
        .dbus_activatable(false)
        .send()
        .await?
        .response()?;
    Ok(())
}

#[cfg(target_os = "linux")]
mod xdg {
    use std::path::{Path, PathBuf};

    use tauri::{AppHandle, Manager, Runtime};

    use super::ARGS;
    use crate::APP_ID;

    /// `$XDG_CONFIG_HOME/autostart/<app id>.desktop`.
    pub fn entry_path(config_dir: &Path) -> PathBuf {
        config_dir
            .join("autostart")
            .join(format!("{APP_ID}.desktop"))
    }

    pub fn apply<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> anyhow::Result<()> {
        let config_dir = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
        let path = entry_path(&config_dir);
        if !enabled {
            match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => return Ok(()),
            }
        }
        // An AppImage runs from a temporary mount; start the image itself.
        let exe = match app.env().appimage {
            Some(appimage) => PathBuf::from(appimage),
            None => std::env::current_exe()?,
        };
        let exe = exe.to_string_lossy();
        // A line break would start a new key in the desktop entry.
        anyhow::ensure!(
            !exe.contains(['\n', '\r']),
            "executable path contains a line break"
        );
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, entry(&exe))?;
        Ok(())
    }

    /// Quote an `Exec` argument as the Desktop Entry spec requires; a
    /// literal `%` is doubled so it is not read as a field code.
    fn quote(arg: &str) -> String {
        let arg = arg.replace('%', "%%");
        let arg = arg.as_str();
        if !arg.contains(|c: char| c.is_whitespace() || "\"'\\><~|&;$*?#()`".contains(c)) {
            return arg.to_owned();
        }
        let mut out = String::from("\"");
        for c in arg.chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
        // `\` is also an escape in the string value itself.
        out.replace('\\', "\\\\")
    }

    pub fn entry(exe: &str) -> String {
        let exec = std::iter::once(quote(exe))
            .chain(ARGS.iter().map(|a| quote(a)))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "[Desktop Entry]\n\
             Version=1.0\n\
             Name=FastZap\n\
             Comment=WhatsApp Desktop for Linux\n\
             Exec={exec}\n\
             Icon={APP_ID}\n\
             Type=Application\n\
             Categories=Chat;Network;InstantMessaging;\n\
             StartupWMClass=fastzap\n\
             Terminal=false\n\
             X-GNOME-UsesNotifications=true\n"
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn entry_starts_hidden_and_quotes_the_path() {
            let text = entry("/opt/Fast Zap/fastzap");
            assert!(text.contains("Exec=\"/opt/Fast Zap/fastzap\" --hideStart\n"));
            assert!(entry("/usr/bin/fastzap").contains("Exec=/usr/bin/fastzap --hideStart\n"));
            assert!(entry("/opt/100%/fastzap").contains("Exec=/opt/100%%/fastzap --hideStart\n"));
            let text = entry("/tmp/a$b");
            assert!(text.contains(r#"Exec="/tmp/a\\$b" --hideStart"#));
        }

        #[test]
        fn entry_lives_in_the_xdg_autostart_dir() {
            assert_eq!(
                entry_path(Path::new("/home/u/.config")),
                PathBuf::from("/home/u/.config/autostart/io.github.quantmind_br.fastzap.desktop")
            );
        }
    }
}

/// Serializes reconciliations so an older request cannot finish last.
static RECONCILE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Apply the preference without blocking the caller. `enabled` is only the
/// startup value; the preference is reread once earlier work has finished.
pub fn sync<R: Runtime>(app: &AppHandle<R>, enabled: bool) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _guard = RECONCILE.lock().await;
        let enabled = app
            .try_state::<crate::app::state::AppState>()
            .map_or(enabled, |state| state.settings.get().system.start_system);
        if let Err(e) = apply(&app, enabled).await {
            log::warn!("failed to update autostart: {e}");
        }
    });
}
