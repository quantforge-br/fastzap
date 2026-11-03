//! WebKitGTK: spell checker languages and the page context menu.

use gtk::gio;
use gtk::glib::ToVariant;
use tauri::{Manager, Runtime, Webview};
use webkit2gtk::{
    ContextMenu, ContextMenuAction, ContextMenuExt, ContextMenuItem, ContextMenuItemExt,
    WebContextExt, WebViewExt,
};

use crate::app::state::AppState;
use crate::i18n::tr;

/// Navigation, link-opening and inspector entries that make no sense in an
/// app window (ZapZap removes the same ones from Qt's menu).
const REMOVED: &[ContextMenuAction] = &[
    ContextMenuAction::GoBack,
    ContextMenuAction::GoForward,
    ContextMenuAction::Stop,
    ContextMenuAction::Reload,
    ContextMenuAction::OpenLink,
    ContextMenuAction::OpenLinkInNewWindow,
    ContextMenuAction::DownloadLinkToDisk,
    ContextMenuAction::OpenImageInNewWindow,
    ContextMenuAction::CopyImageUrlToClipboard,
    ContextMenuAction::OpenFrameInNewWindow,
    ContextMenuAction::OpenVideoInNewWindow,
    ContextMenuAction::OpenAudioInNewWindow,
    ContextMenuAction::InspectElement,
];

pub fn apply_to<R: Runtime>(webview: &Webview<R>, enabled: bool, languages: Vec<String>) {
    let _ = webview.with_webview(move |platform| {
        let Some(context) = platform.inner().context() else {
            return;
        };
        let refs: Vec<&str> = languages.iter().map(String::as_str).collect();
        context.set_spell_checking_languages(&refs);
        context.set_spell_checking_enabled(enabled);
    });
}

pub fn install_context_menu<R: Runtime>(webview: &Webview<R>) {
    let app = webview.app_handle().clone();
    let (enabled, languages) = super::current(&app);
    apply_to(webview, enabled, languages);
    let _ = webview.with_webview(move |platform| {
        platform.inner().connect_context_menu(move |_, menu, _, _| {
            customize(&app, menu);
            false
        });
    });
}

fn customize<R: Runtime>(app: &tauri::AppHandle<R>, menu: &ContextMenu) {
    for item in menu.items() {
        if REMOVED.contains(&item.stock_action()) {
            menu.remove(&item);
        }
    }
    tidy_separators(menu);

    let enabled = app.state::<AppState>().settings.get().spellcheck.enabled;
    let toggle =
        gio::SimpleAction::new_stateful("fastzap-check-spelling", None, &enabled.to_variant());
    let handle = app.clone();
    toggle.connect_activate(move |_, _| {
        let _ = crate::app::effects::change_settings(&handle, |s| {
            s.spellcheck.enabled = !s.spellcheck.enabled;
        });
    });
    let languages = gio::SimpleAction::new("fastzap-spellcheck-languages", None);
    languages.set_enabled(enabled);
    let handle = app.clone();
    languages.connect_activate(move |_, _| {
        crate::app::actions::dispatch(&handle, "spellcheck_languages");
    });

    if menu.n_items() > 0 {
        menu.append(&ContextMenuItem::new_separator());
    }
    menu.append(&ContextMenuItem::from_gaction(
        &toggle,
        &tr("Check Spelling"),
        None,
    ));
    menu.append(&ContextMenuItem::from_gaction(
        &languages,
        &tr("Languages…"),
        None,
    ));
}

/// Drop leading, trailing and repeated separators left by the removals.
fn tidy_separators(menu: &ContextMenu) {
    let mut previous_separator = true;
    for item in menu.items() {
        let separator = item.is_separator();
        if separator && previous_separator {
            menu.remove(&item);
        } else {
            previous_separator = separator;
        }
    }
    if let Some(last) = menu.last()
        && last.is_separator()
    {
        menu.remove(&last);
    }
}
