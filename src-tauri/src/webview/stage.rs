//! Linux layout stage for child webviews.
//!
//! On Linux, Tauri packs every child webview into the window's vertical
//! `GtkBox`, so `set_position` has no effect and webviews stack. FastZap moves
//! them into a `GtkOverlay`: the local UI is the main child (it fills the
//! window) and each account webview is an overlay child placed by margins.

use gtk::prelude::*;
use tauri::{Runtime, Webview, Window};

use super::manager::Rect;

/// Insert the overlay into the window; call before adding any webview.
pub fn install<R: Runtime>(window: &Window<R>) -> tauri::Result<()> {
    let vbox = window.default_vbox()?;
    let overlay = gtk::Overlay::new();
    vbox.pack_start(&overlay, true, true, 0);
    overlay.show();
    Ok(())
}

fn move_into_overlay(widget: &gtk::Widget, as_main: bool) {
    let Some(vbox) = widget.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) else {
        return;
    };
    let Some(overlay) = vbox
        .children()
        .into_iter()
        .find_map(|c| c.downcast::<gtk::Overlay>().ok())
    else {
        log::warn!("webview stage overlay missing");
        return;
    };
    vbox.remove(widget);
    if as_main {
        overlay.add(widget);
    } else {
        widget.set_halign(gtk::Align::Start);
        widget.set_valign(gtk::Align::Start);
        overlay.add_overlay(widget);
    }
}

/// Make the local UI webview the overlay's main child.
pub fn attach_main<R: Runtime>(webview: &Webview<R>) {
    let _ = webview.with_webview(|platform| {
        move_into_overlay(platform.inner().upcast_ref(), true);
    });
}

/// Move an account webview above the UI.
pub fn attach_child<R: Runtime>(webview: &Webview<R>) {
    let _ = webview.with_webview(|platform| {
        move_into_overlay(platform.inner().upcast_ref(), false);
    });
}

pub fn place<R: Runtime>(webview: &Webview<R>, rect: Rect) {
    let _ = webview.with_webview(move |platform| {
        let widget: gtk::Widget = platform.inner().upcast();
        widget.set_margin_start(rect.x.round() as i32);
        widget.set_margin_top(rect.y.round() as i32);
        widget.set_size_request(
            rect.width.round().max(1.0) as i32,
            rect.height.round().max(1.0) as i32,
        );
    });
}
