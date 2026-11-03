//! Scripts injected into WhatsApp Web pages.

use crate::settings::{PdfClickBehavior, Settings};

const BRIDGE: &str = include_str!("scripts/bridge.js");
const CTRL_ARROW_FIX: &str = include_str!("scripts/ctrl_arrow_visual_navigation_fix.js");
const WEBRTC_SHIELD: &str = include_str!("scripts/webrtc_shield.js");
const STRICT_PROXY: &str = include_str!("scripts/strict_proxy.js");
const THEME_CONTROLLER: &str = include_str!("scripts/theme_controller.js");
const PDF_CLICK_CONTROLLER: &str = include_str!("scripts/pdf_click_controller.js");
const RESIZABLE_CHAT_LIST: &str = include_str!("scripts/resizable_chat_list.js");
const DISABLE_ANIMATIONS: &str = include_str!("scripts/disable_animations.js");

const VIEW_GRID_ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" fill="currentColor" viewBox="0 0 256 256"><path d="M104,40H48A8,8,0,0,0,40,48v56a8,8,0,0,0,8,8h56a8,8,0,0,0,8-8V48A8,8,0,0,0,104,40Zm-8,48H56V56h40ZM208,40H152a8,8,0,0,0-8,8v56a8,8,0,0,0,8,8h56a8,8,0,0,0,8-8V48A8,8,0,0,0,208,40Zm-8,48H160V56h40ZM104,144H48a8,8,0,0,0-8,8v56a8,8,0,0,0,8,8h56a8,8,0,0,0,8-8V152A8,8,0,0,0,104,144Zm-8,48H56V160h40ZM208,144H152a8,8,0,0,0-8,8v56a8,8,0,0,0,8,8h56a8,8,0,0,0,8-8V152A8,8,0,0,0,208,144Zm-8,48H160V160h40Z"></path></svg>"#;

/// Labels shown inside WhatsApp Web; translated by the caller.
pub struct PageLabels {
    pub selector: String,
    pub selector_activity: String,
    pub resize_handle: String,
    pub resize_hint: String,
}

fn json(value: &str) -> String {
    serde_json::to_string(value).expect("string serializes")
}

/// Script registered at document creation. Changes take effect on the next load.
pub fn document_start(settings: &Settings) -> String {
    let mut out = String::from(BRIDGE);
    if settings.performance.ctrl_arrow_visual_navigation_fix {
        out.push('\n');
        out.push_str(CTRL_ARROW_FIX);
    }
    if settings.privacy.webrtc_shield {
        out.push('\n');
        out.push_str(WEBRTC_SHIELD);
    }
    if settings.performance.disable_animations {
        out.push('\n');
        out.push_str(DISABLE_ANIMATIONS);
    }
    out
}

/// Whether WebRTC must be disabled because it would bypass the proxy.
pub fn strict_proxy(settings: &Settings) -> bool {
    settings.privacy.strict_proxy && settings.proxy.enabled
}

/// Scripts injected into every frame, so a same-origin iframe cannot hand
/// the page an untouched API.
pub fn all_frames(settings: &Settings) -> Option<&'static str> {
    strict_proxy(settings).then_some(STRICT_PROXY)
}

/// Scripts evaluated after each WhatsApp Web load (ZapZap's DocumentReady scripts).
pub fn document_ready(settings: &Settings, dark: bool, labels: &PageLabels) -> String {
    let theme = THEME_CONTROLLER
        .replace(
            "{quick_accounts_visible}",
            if settings.appearance.integrated_account_selector_enabled {
                "true"
            } else {
                "false"
            },
        )
        .replace("{quick_accounts_icon}", VIEW_GRID_ICON)
        .replace("{selector_label}", &json(&labels.selector))
        .replace(
            "{selector_activity_label}",
            &json(&labels.selector_activity),
        )
        .replace(
            "{current_color_scheme}",
            if dark { "dark" } else { "light" },
        );
    let pdf_mode = match settings.downloads.pdf_click_behavior {
        PdfClickBehavior::Preview => "\"preview\"",
        PdfClickBehavior::Download => "\"download\"",
    };
    let mut out = theme;
    out.push('\n');
    out.push_str(&PDF_CLICK_CONTROLLER.replace("__ZAPZAP_PDF_CLICK_MODE__", pdf_mode));
    if settings.appearance.resizable_chat_list_enabled {
        out.push('\n');
        out.push_str(&resizable_chat_list(labels));
    }
    out
}

pub fn resizable_chat_list(labels: &PageLabels) -> String {
    RESIZABLE_CHAT_LIST
        .replace("{resize_handle_label}", &json(&labels.resize_handle))
        .replace("{resize_handle_hint}", &json(&labels.resize_hint))
}

pub fn disable_resizable_chat_list() -> &'static str {
    "window._zapZapResizableChatList && window._zapZapResizableChatList.destroy();"
}

pub fn set_pdf_click_mode(mode: PdfClickBehavior) -> String {
    let mode = match mode {
        PdfClickBehavior::Preview => "preview",
        PdfClickBehavior::Download => "download",
    };
    format!("window.__zapzapPdfClicks && window.__zapzapPdfClicks.setMode({mode:?});")
}

/// Apply the FastZap color scheme through the ported theme controller.
pub fn apply_theme(dark: bool) -> String {
    let scheme = if dark { "dark" } else { "light" };
    format!(
        r#"(() => {{
  try {{
    const c = window._zapZapWAWebThemeController;
    if (!c || typeof c.is_ready !== "function" || c.has_failed() || !c.is_ready()) return;
    c.currentColorScheme = "{scheme}";
    c.applyZapZapColorSchemeToWAWeb();
  }} catch (e) {{ console.error("[ZapZap WAWeb Theme Controller]", e); }}
}})();"#
    )
}

/// Fallback when the controller fails: force WhatsApp's light theme so the
/// engine-level dark mode (if any) can invert it, then reload.
pub fn theme_fallback() -> &'static str {
    r#"(() => {
  localStorage["theme"] = JSON.stringify("light");
  localStorage["system-theme-mode"] = JSON.stringify(false);
})();"#
}

pub fn set_account_selector_state(enabled: bool, has_other_unread: bool) -> String {
    format!(
        "window._zapZapQuickAccountsController && window._zapZapQuickAccountsController.setState({enabled}, {has_other_unread});"
    )
}

pub fn paste_plain_text(text: &str) -> String {
    format!(
        "window.__FASTZAP__ && window.__FASTZAP__.pastePlainText({});",
        json(text)
    )
}

pub fn notification_clicked(id: u64) -> String {
    format!("window.__FASTZAP__ && window.__FASTZAP__.notificationClicked({id});")
}

pub fn read_profile_photo() -> &'static str {
    r#"(() => {
  const f = window.__FASTZAP__;
  if (!f) return;
  f.invoke("wa_profile_photo", { url: f.readProfilePhoto() });
})();"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_proxy_needs_an_enabled_proxy() {
        let mut settings = Settings::default();
        settings.privacy.strict_proxy = true;
        assert!(all_frames(&settings).is_none());
        settings.proxy.enabled = true;
        assert!(all_frames(&settings).unwrap().contains("RTCPeerConnection"));
        assert!(!document_start(&settings).contains("RTCRtpSender"));
    }

    fn labels() -> PageLabels {
        PageLabels {
            selector: "Account \"selector\"".into(),
            selector_activity: "Notifications".into(),
            resize_handle: "Chat list width".into(),
            resize_hint: "Drag".into(),
        }
    }

    #[test]
    fn document_ready_fills_every_placeholder() {
        let mut settings = Settings::default();
        settings.appearance.resizable_chat_list_enabled = true;
        let script = document_ready(&settings, true, &labels());
        for placeholder in [
            "{quick_accounts_visible}",
            "{quick_accounts_icon}",
            "{selector_label}",
            "{selector_activity_label}",
            "{current_color_scheme}",
            "__ZAPZAP_PDF_CLICK_MODE__",
            "{resize_handle_label}",
            "{resize_handle_hint}",
        ] {
            assert!(
                !script.contains(placeholder),
                "{placeholder} left in script"
            );
        }
        assert!(script.contains(r#"Account \"selector\""#));
        assert!(script.contains(r#"currentColorScheme: "dark""#));
    }

    #[test]
    fn document_start_respects_toggles() {
        let mut settings = Settings::default();
        assert!(document_start(&settings).contains("__FASTZAP__"));
        assert!(!document_start(&settings).contains("candidate-obfuscation"));
        settings.privacy.webrtc_shield = true;
        settings.performance.ctrl_arrow_visual_navigation_fix = false;
        let script = document_start(&settings);
        assert!(script.contains("candidate-obfuscation"));
        assert!(!script.contains("CtrlArrowVisualNavigationFix"));
    }

    #[test]
    fn paste_text_is_json_escaped() {
        assert!(paste_plain_text("a\"b\n").contains(r#"pastePlainText("a\"b\n")"#));
    }
}
