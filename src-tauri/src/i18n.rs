//! Interface translations shared by native surfaces (tray, menus, notifications,
//! strings injected into WhatsApp Web). Catalogs are generated from `po/*.po`
//! into `src/lib/i18n/catalogs/*.json` and embedded at build time.

use std::collections::HashMap;
use std::sync::RwLock;

include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));

static ACTIVE: RwLock<Option<HashMap<&'static str, &'static str>>> = RwLock::new(None);

/// Resolve `"system"` to the OS locale, normalized to `ll_CC` form.
pub fn resolve_locale(setting: &str) -> String {
    let raw = if setting.is_empty() || setting == "system" {
        sys_locale()
    } else {
        setting.to_owned()
    };
    raw.split('.').next().unwrap_or("en").replace('-', "_")
}

fn sys_locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
        .unwrap_or_else(|| "en".into())
}

/// Pick the best catalog for a locale: exact (`pt_BR`), then language (`pt`).
pub fn catalog_for(locale: &str) -> Option<&'static str> {
    let lang = locale.split('_').next().unwrap_or(locale);
    CATALOGS
        .iter()
        .find(|(code, _)| *code == locale)
        .or_else(|| CATALOGS.iter().find(|(code, _)| *code == lang))
        .or_else(|| {
            CATALOGS
                .iter()
                .find(|(code, _)| code.split('_').next() == Some(lang))
        })
        .map(|(_, json)| *json)
}

pub fn set_language(setting: &str) {
    let locale = resolve_locale(setting);
    let map = catalog_for(&locale)
        .and_then(|json| serde_json::from_str::<HashMap<&'static str, &'static str>>(json).ok());
    *ACTIVE.write().unwrap() = map;
}

pub fn available_languages() -> Vec<&'static str> {
    CATALOGS.iter().map(|(code, _)| *code).collect()
}

/// Translate an English source string; falls back to the source.
pub fn tr(source: &str) -> String {
    ACTIVE
        .read()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(source).copied())
        .filter(|t| !t.is_empty())
        .unwrap_or(source)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_resolution() {
        assert_eq!(resolve_locale("pt_BR.UTF-8"), "pt_BR");
        assert_eq!(resolve_locale("pt-BR"), "pt_BR");
    }

    #[test]
    fn untranslated_strings_fall_back_to_source() {
        set_language("xx_YY");
        assert_eq!(tr("Quit"), "Quit");
    }
}
