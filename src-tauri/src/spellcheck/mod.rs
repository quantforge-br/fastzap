//! Spell checking (ZapZap's `dictionaries` feature) on Enchant + Hunspell:
//! language selection, dictionary installs, provisioning for the system
//! locale and, on Linux, WebKit's context menu.

pub mod catalog;
#[cfg(target_os = "linux")]
mod linux;
pub mod store;

use std::collections::HashSet;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use ts_rs::TS;

use crate::app::state::AppState;
use crate::settings::{
    MAX_RECENT_SPELLCHECK_LANGUAGES, MAX_SPELLCHECK_LANGUAGES, SpellcheckSettings,
};
use catalog::{catalog, enchant_tag};
use store::SpellError;

#[cfg(target_os = "linux")]
pub use linux::{apply_to, install_context_menu};

pub const SPELLCHECK_CHANGED: &str = "spellcheck://changed";

/// Downloads in progress, by tag.
#[derive(Default)]
pub struct Spellcheck {
    busy: Mutex<HashSet<String>>,
}

/// The installed tag that serves `wanted`: the exact tag, the bare language
/// (`pt_BR` → `pt`), or another variant of the language.
pub fn match_locale<'a>(
    tags: impl IntoIterator<Item = &'a String>,
    wanted: &str,
) -> Option<String> {
    let wanted = enchant_tag(wanted);
    let lang = wanted.split('_').next().unwrap_or(&wanted).to_owned();
    let tags: Vec<&String> = tags.into_iter().collect();
    tags.iter()
        .find(|t| **t == &wanted)
        .or_else(|| tags.iter().find(|t| **t == &lang))
        .or_else(|| {
            tags.iter()
                .find(|t| t.split('_').next() == Some(lang.as_str()))
        })
        .map(|t| (*t).clone())
}

/// Selected languages resolved to installed tags, deduplicated, in order.
pub fn normalize_selection(selected: &[String], installed: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for code in selected {
        if let Some(tag) = match_locale(installed, code)
            && !out.contains(&tag)
        {
            out.push(tag);
        }
    }
    out.truncate(MAX_SPELLCHECK_LANGUAGES);
    out
}

/// The languages actually used: the selection, else the dictionary of the
/// system locale, else the first installed one.
pub fn effective_languages(
    settings: &SpellcheckSettings,
    installed: &[String],
    locale: &str,
) -> Vec<String> {
    let selected = normalize_selection(&settings.languages, installed);
    if !selected.is_empty() {
        return selected;
    }
    match_locale(installed, locale)
        .or_else(|| installed.first().cloned())
        .into_iter()
        .collect()
}

/// ZapZap's recents after an Apply: newly selected first, then the rest of
/// the selection, then the older recents.
pub fn push_recent(recent: &[String], previous: &[String], selection: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let fresh = selection.iter().filter(|c| !previous.contains(c));
    let rest = selection.iter().filter(|c| previous.contains(c));
    for code in fresh.chain(rest).chain(recent) {
        if !out.contains(code) {
            out.push(code.clone());
        }
    }
    out.truncate(MAX_RECENT_SPELLCHECK_LANGUAGES);
    out
}

pub fn system_locale() -> String {
    enchant_tag(&crate::i18n::resolve_locale("system"))
}

fn installed_tags<R: Runtime>(app: &AppHandle<R>) -> Vec<String> {
    let root = app.state::<AppState>().paths.dictionaries_dir();
    store::installed(&root).into_keys().collect()
}

/// Enabled flag and languages for the account pages.
pub fn current<R: Runtime>(app: &AppHandle<R>) -> (bool, Vec<String>) {
    let settings = app.state::<AppState>().settings.get().spellcheck;
    let languages = effective_languages(&settings, &installed_tags(app), &system_locale());
    (settings.enabled, languages)
}

/// Apply the settings to every account page.
pub fn apply_all<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "linux")]
    {
        let (enabled, languages) = current(app);
        for (label, webview) in app.webviews() {
            if crate::webview::manager::account_id_from_label(&label).is_some() {
                apply_to(&webview, enabled, languages.clone());
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = app;
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct InstalledDictionary {
    pub tag: String,
    /// In FastZap's store (removable), not a system directory.
    pub managed: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct AvailableDictionary {
    pub tag: String,
    #[ts(type = "number")]
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SpellState {
    /// Whether FastZap drives the engine's spell checker on this platform.
    pub supported: bool,
    pub enabled: bool,
    /// Languages in use (the selection, or its fallback).
    pub languages: Vec<String>,
    pub recent: Vec<String>,
    pub installed: Vec<InstalledDictionary>,
    pub available: Vec<AvailableDictionary>,
    pub busy: Vec<String>,
    pub catalog_revision: String,
}

pub fn state<R: Runtime>(app: &AppHandle<R>) -> SpellState {
    let app_state = app.state::<AppState>();
    let settings = app_state.settings.get().spellcheck;
    let installed = store::installed(&app_state.paths.dictionaries_dir());
    let tags: Vec<String> = installed.keys().cloned().collect();
    let c = catalog();
    let mut busy: Vec<String> = app_state
        .spellcheck
        .busy
        .lock()
        .unwrap()
        .iter()
        .cloned()
        .collect();
    busy.sort();
    SpellState {
        supported: cfg!(target_os = "linux"),
        enabled: settings.enabled,
        languages: effective_languages(&settings, &tags, &system_locale()),
        recent: normalize_selection(&settings.recent_languages, &tags),
        installed: installed
            .into_iter()
            .map(|(tag, managed)| InstalledDictionary { tag, managed })
            .collect(),
        available: c
            .dictionaries
            .iter()
            .map(|e| AvailableDictionary {
                tag: e.tag(),
                size: e.aff.size + e.dic.size,
            })
            .collect(),
        busy,
        catalog_revision: c.commit[..7].to_owned(),
    }
}

fn notify<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit(SPELLCHECK_CHANGED, ());
}

/// Download a catalog dictionary, then refresh the pages.
pub async fn install<R: Runtime>(app: &AppHandle<R>, tag: &str) -> Result<(), SpellError> {
    let c = catalog();
    let entry = c.find(tag).ok_or(SpellError::InvalidFile)?;
    let root = {
        let state = app.state::<AppState>();
        if !state.spellcheck.busy.lock().unwrap().insert(tag.to_owned()) {
            return Err(SpellError::Busy);
        }
        state.paths.dictionaries_dir()
    };
    notify(app);
    let result = store::install(&root, c, entry).await;
    app.state::<AppState>()
        .spellcheck
        .busy
        .lock()
        .unwrap()
        .remove(tag);
    if result.is_ok() {
        apply_all(app);
    }
    notify(app);
    result
}

/// Once per system locale, install its catalog dictionary when none is
/// installed (ZapZap's provisioning). Failures are logged and retried on
/// the next start.
pub fn provision<R: Runtime>(app: &AppHandle<R>) {
    let locale = system_locale();
    let settings = app.state::<AppState>().settings.get().spellcheck;
    if settings.provisioned_locale.as_deref() == Some(locale.as_str()) {
        return;
    }
    let mark = |app: &AppHandle<R>, locale: String| {
        let _ = crate::app::effects::change_settings(app, |s| {
            s.spellcheck.provisioned_locale = Some(locale);
        });
    };
    if match_locale(&installed_tags(app), &locale).is_some() {
        return mark(app, locale);
    }
    let tags: Vec<String> = catalog().dictionaries.iter().map(|e| e.tag()).collect();
    let Some(tag) = match_locale(&tags, &locale) else {
        log::info!("no catalog dictionary for locale {locale}");
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match install(&app, &tag).await {
            Ok(()) => mark(&app, locale),
            Err(e) => log::warn!("could not provision dictionary {tag}: {e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn locales_match_exact_then_language() {
        let installed = tags(&["en", "en_GB", "pt", "pt_PT"]);
        assert_eq!(match_locale(&installed, "pt_PT").as_deref(), Some("pt_PT"));
        assert_eq!(match_locale(&installed, "pt_BR").as_deref(), Some("pt"));
        assert_eq!(match_locale(&installed, "en-gb").as_deref(), Some("en_GB"));
        assert_eq!(
            match_locale(&tags(&["de_AT"]), "de_DE").as_deref(),
            Some("de_AT")
        );
        assert_eq!(match_locale(&installed, "fr_FR"), None);
    }

    #[test]
    fn selection_falls_back_like_zapzap() {
        let installed = tags(&["de", "en", "pt"]);
        let mut s = SpellcheckSettings {
            languages: tags(&["pt_BR", "pt", "xx", "en"]),
            ..Default::default()
        };
        assert_eq!(effective_languages(&s, &installed, "fr"), ["pt", "en"]);
        s.languages.clear();
        assert_eq!(effective_languages(&s, &installed, "en_US"), ["en"]);
        assert_eq!(effective_languages(&s, &installed, "fr_FR"), ["de"]);
        assert!(effective_languages(&s, &[], "en").is_empty());
    }

    #[test]
    fn recents_put_new_choices_first() {
        let recent = push_recent(
            &tags(&["fr", "it"]),
            &tags(&["en"]),
            &tags(&["en", "pt", "de"]),
        );
        assert_eq!(recent, ["pt", "de", "en", "fr"]);
    }
}
