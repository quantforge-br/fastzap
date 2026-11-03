//! User CSS/JS customizations injected into WhatsApp Web, with ZapZap's
//! folder layout (`customizations/{global,accounts/<key>}/{css,js}`) and
//! rules: global files always apply when enabled; an account that does not
//! inherit adds its own files after them.

pub mod files;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, Runtime, Webview};
use ts_rs::TS;

use crate::accounts::DEFAULT_ACCOUNT_ID;
use crate::app::state::AppState;
use crate::settings::{AssetToggle, CustomizationSettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AssetKind {
    Css,
    Js,
}

impl AssetKind {
    pub fn ext(self) -> &'static str {
        match self {
            AssetKind::Css => "css",
            AssetKind::Js => "js",
        }
    }
}

/// `None` is the global scope.
pub type Scope = Option<i64>;

/// ZapZap's per-account key: the default account is `storage-whats`.
pub fn account_key(id: i64) -> String {
    if id == DEFAULT_ACCOUNT_ID {
        "storage-whats".into()
    } else {
        id.to_string()
    }
}

pub fn scope_dir(base: &Path, scope: Scope) -> PathBuf {
    match scope {
        None => base.join("global"),
        Some(id) => base.join("accounts").join(account_key(id)),
    }
}

pub fn toggle(settings: &CustomizationSettings, scope: Scope, kind: AssetKind) -> AssetToggle {
    let pick = |css: &AssetToggle, js: &AssetToggle| match kind {
        AssetKind::Css => css.clone(),
        AssetKind::Js => js.clone(),
    };
    match scope {
        None => pick(&settings.global.css, &settings.global.js),
        Some(id) => settings
            .accounts
            .get(&account_key(id))
            .map(|a| pick(&a.css, &a.js))
            .unwrap_or_default(),
    }
}

pub struct Entry {
    pub key: String,
    pub content: String,
}

fn scope_entries(base: &Path, scope: Scope, kind: AssetKind, toggle: &AssetToggle) -> Vec<Entry> {
    if !toggle.enabled {
        return Vec::new();
    }
    let prefix = match scope {
        None => "global".to_string(),
        Some(id) => format!("account-{}", account_key(id)),
    };
    let dir = scope_dir(base, scope).join(kind.ext());
    files::list(&dir, kind)
        .into_iter()
        .filter(|name| !toggle.disabled_files.contains(name))
        .filter_map(|name| {
            let content = std::fs::read_to_string(dir.join(&name)).ok()?;
            (!content.trim().is_empty()).then(|| Entry {
                key: format!("{prefix}:file:{name}"),
                content,
            })
        })
        .collect()
}

/// Files applied to an account, in injection order.
pub fn effective_entries(
    base: &Path,
    settings: &CustomizationSettings,
    account_id: i64,
    kind: AssetKind,
) -> Vec<Entry> {
    let mut entries = scope_entries(base, None, kind, &toggle(settings, None, kind));
    let inherit = settings
        .accounts
        .get(&account_key(account_id))
        .is_none_or(|a| a.inherit);
    if !inherit {
        let scope = Some(account_id);
        entries.extend(scope_entries(
            base,
            scope,
            kind,
            &toggle(settings, scope, kind),
        ));
    }
    entries
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum UserstyleError {
    #[error("This userstyle does not target WhatsApp Web.")]
    InvalidTarget,
}

/// Keep the bodies of `@-moz-document` blocks that target whatsapp.com.
/// `strict` rejects userstyles without such a block; otherwise the raw text
/// is used, as ZapZap does at injection time.
pub fn normalize_userstyle(text: &str, strict: bool) -> Result<String, UserstyleError> {
    if !text.contains("@-moz-document") {
        return Ok(text.to_string());
    }
    let mut kept = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("@-moz-document") {
        let after = &rest[at..];
        let Some(open) = after.find('{') else { break };
        let header = after[..open].to_lowercase();
        let body_start = open + 1;
        let mut depth = 1;
        let mut end = None;
        for (i, c) in after[body_start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(body_start + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else { break };
        if header.contains("whatsapp.com") {
            kept.push(after[body_start..end].trim().to_string());
        }
        rest = &after[end + 1..];
    }
    if kept.is_empty() {
        return if strict {
            Err(UserstyleError::InvalidTarget)
        } else {
            Ok(text.to_string())
        };
    }
    Ok(kept.join("\n\n"))
}

/// Script replacing FastZap's custom `<style>` tags with `entries`.
pub fn css_script(entries: &[Entry]) -> String {
    let styles: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            let hash = hex::encode(Sha256::digest(e.key.as_bytes()));
            serde_json::json!({
                "id": format!("zapzap-custom-css-{}", &hash[..12]),
                "css": normalize_userstyle(&e.content, false).unwrap_or_default(),
            })
        })
        .collect();
    format!(
        r#"(() => {{
  const styles = {};
  document.querySelectorAll("style[data-zapzap-custom-css]").forEach((s) => s.remove());
  const parent = document.head || document.documentElement;
  if (!parent) return;
  for (const s of styles) {{
    const el = document.createElement("style");
    el.id = s.id;
    el.dataset.zapzapCustomCss = "1";
    el.textContent = s.css;
    parent.appendChild(el);
  }}
}})();"#,
        serde_json::Value::Array(styles)
    )
}

fn js_source(entry: &Entry) -> String {
    format!(
        "{}\n//# sourceURL=zapzap-custom-js/{}",
        entry.content, entry.key
    )
}

/// Inject CSS and run JS after each page load, like ZapZap.
pub fn apply_to_webview<R: Runtime>(app: &AppHandle<R>, account_id: i64, webview: &Webview<R>) {
    let state = app.state::<AppState>();
    let base = state.paths.customizations_dir();
    let settings = state.settings.get().customizations;
    let css = effective_entries(&base, &settings, account_id, AssetKind::Css);
    let _ = webview.eval(css_script(&css));
    for entry in effective_entries(&base, &settings, account_id, AssetKind::Js) {
        let _ = webview.eval(js_source(&entry));
    }
}

/// Settings changed: refresh the CSS of open pages. Scripts only run on the
/// next load, so they are not run twice.
pub fn reapply_all<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let base = state.paths.customizations_dir();
    let settings = state.settings.get().customizations;
    for (label, webview) in app.webviews() {
        if let Some(id) = crate::webview::manager::account_id_from_label(&label) {
            let css = effective_entries(&base, &settings, id, AssetKind::Css);
            let _ = webview.eval(css_script(&css));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AccountCustomization;

    #[test]
    fn userstyles_keep_whatsapp_blocks() {
        let text = "a{} @-moz-document domain(\"web.whatsapp.com\") { b { c: d; } } @-moz-document domain(\"x.org\") { e{} }";
        assert_eq!(normalize_userstyle(text, true).unwrap(), "b { c: d; }");
        let other = "@-moz-document domain(\"x.org\") { e{} }";
        assert_eq!(
            normalize_userstyle(other, true),
            Err(UserstyleError::InvalidTarget)
        );
        assert_eq!(normalize_userstyle(other, false).unwrap(), other);
        assert_eq!(normalize_userstyle("plain{}", true).unwrap(), "plain{}");
    }

    #[test]
    fn accounts_extend_global_unless_they_inherit() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        for (scope, name) in [
            (None, "b.css"),
            (None, "A.css"),
            (Some(2), "acc.css"),
            (None, "off.css"),
        ] {
            let d = scope_dir(base, scope).join("css");
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(name), "x{}").unwrap();
        }
        std::fs::write(scope_dir(base, None).join("css/empty.css"), "  ").unwrap();
        let mut settings = CustomizationSettings::default();
        settings.global.css.enabled = true;
        settings.global.css.disabled_files = vec!["off.css".into()];
        let keys = |s: &CustomizationSettings| -> Vec<String> {
            effective_entries(base, s, 2, AssetKind::Css)
                .into_iter()
                .map(|e| e.key)
                .collect()
        };
        assert_eq!(keys(&settings), ["global:file:A.css", "global:file:b.css"]);
        let mut account = AccountCustomization {
            inherit: false,
            ..Default::default()
        };
        account.css.enabled = true;
        settings.accounts.insert("2".into(), account);
        assert_eq!(
            keys(&settings),
            [
                "global:file:A.css",
                "global:file:b.css",
                "account-2:file:acc.css"
            ]
        );
        assert_eq!(account_key(1), "storage-whats");
    }

    #[test]
    fn css_script_embeds_json() {
        let script = css_script(&[Entry {
            key: "global:file:a.css".into(),
            content: "</style><script>".into(),
        }]);
        assert!(script.contains(r#""css":"</style><script>""#));
        assert!(script.contains("zapzap-custom-css-"));
    }
}
