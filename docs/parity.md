# Parity with ZapZap

Reference: ZapZap 7.5.2 (`~/dev/zapzap`). Each row names the origin module,
the equivalent FastZap module, and intentional differences.

| Feature | ZapZap | FastZap | Notes |
|---|---|---|---|
| Single instance and link forwarding | `app/single_application.py` | `tauri-plugin-single-instance` + `app::lifecycle` | `fastzap://restart` replaces `zapzap://restart` |
| D-Bus `org.freedesktop.Application` | `app/desktop_application_dbus.py` | `app::dbus` | Flatpak only |
| CLI `--setSettings`, `--wayland`, `--hideStart` | `app/startup_options.py` | `app::cli` | same names |
| `SIGTERM` → clean exit | `app/unix_signal_bridge.py` | `app::lifecycle` | |
| Geometry, close to tray, confirmation | `app/window_lifecycle.py` | `app::lifecycle` + `tauri-plugin-window-state` | `SIGINT` also quits cleanly |
| Custom title bar (CSR) | `ui/components/client_side_window*` | `src/lib/components/TitleBar.svelte` | same `csr_*` options, applied without restart; opaque 4 px frame instead of the translucent margin with rounded corners; new `appearance.title_bar` ("Show title bar") hides any bar, native or custom |
| Accounts, limit, activation, avatar, UA, DND, zoom | `features/accounts` | `accounts` + Accounts page | new account kind: Evolution Go |
| Sidebar, grid, locking, refresh | `features/browser/shell` | `src/routes/+page.svelte` + `src/lib/accounts/AccountsOverview.svelte` | the grid is the accounts overview (cards, no webviews) and includes Evolution Go accounts |
| Menu bar and interface scale | `ui/main_window`, `appearance.scale` | `MenuBar.svelte` (native menus) + webview zoom | |
| Settings pages | `features/settings` | `src/lib/settings/` | same order and texts |
| Floating monitoring panel | `ui/components/floating_monitoring_panel.py` | `app::panel` + `/panel` route (`panel` window) | position and locking only off Wayland, as in ZapZap |
| Built-in account switcher | `theme_controller.js` (QuickAccounts) | `webview/scripts/quick_accounts.js` | |
| Unread counter from the title | `web_view.py::_on_title_changed` | `webview/scripts/bridge.js` | |
| Reload after page process crash | `web_view.py::_on_render_crash` | `webview::manager` (`web-process-terminated`) | Linux; reloads after 1 s, unless the termination was API-requested |
| Navigation allowlist and popups | `PopupRoutingPage` | `webview::navigation` | |
| Deep links `whatsapp:` / wa.me | `deeplink.py` | `webview::deeplink` | |
| Chat by number (`Ctrl+M`) | `open_chat.py`, `send_message_to_number_dialog.py` | `webview::deeplink` + `src/lib/chat/` | |
| Paste as text and spreadsheet tables | `clipboard_table.py`, `web_view.py` | `webview::clipboard` | |
| Attach files by dragging | `web_view.py` | HTML drag-and-drop on WhatsApp Web (replayed from Tauri events on Linux, `webview/file_drop.rs`); Tauri events + `Conversation.svelte` on Evolution Go | Evolution: multiple files, removal before confirming, optional caption, and 100 MB limit per file; no automatic retry |
| Context menu | `web_view.py` | `spellcheck::linux` (WebKitGTK `context-menu` signal) | Linux; removes navigation/open link/inspect and adds "Check spelling" and "Languages…"; default labels come from WebKit in the system language |
| Custom CSS/JS | `features/customizations` | `customizations` + Advanced customizations page | same file tree; CSS is re-applied to open pages on save |
| Spell checker | `features/dictionaries` (`.bdic`) | `spellcheck` + `src/lib/spellcheck/` | **Hunspell format** (`.dic/.aff`) from `wooorm/dictionaries` at a pinned commit; `.bdic` is not imported; Linux |
| Downloads and history (`Ctrl+J`) | `features/downloads` | `downloads` + `src/lib/downloads/` | decision after the download finishes; Evolution Go chat media follow the same flow and history; see gaps |
| Notifications (portal, D-Bus, Windows, macOS) | `features/notifications` | `notifications` | |
| Tray, icon themes, counter, badge | `features/tray` | `tray` | |
| Shortcuts | `features/shortcuts` | `bridge.js` (pages) + `src/lib/shell/shortcuts.ts` (local UI) + `src/lib/shortcuts/` | "Keyboard shortcuts" dialog; both mappings must stay identical |
| Start with the system | `features/startup` | `autostart` | `RequestBackground` portal on Flatpak; XDG entry named after the App ID on other Linux |
| Permissions (camera, microphone, etc.) | `features/permissions` | `permissions` | Linux; see gaps |
| Proxy and strict proxy | `core/environment/proxy_manager.py` | `proxy` | strict proxy by blocking non-proxied WebRTC |
| WebRTC shield | `webrtc_shield.js` | `webview/scripts/webrtc_shield.js` | |
| `Ctrl+←/→` fix | `ctrl_arrow_visual_navigation_fix.js` | same | |
| PDF click | `pdf_click_controller.js` | same | |
| Resizable chat list | `resizable_chat_list.js` | same | |
| Auto/light/dark theme | `core/theme`, `theme_controller.js` | `theme` + ported script | `auto` follows the `color-scheme` portal |
| Interface language | gettext `po/` | `i18n` (JSON catalogs generated from the `.po` files) | |
| Update check | `core/update_checker.py` | `update` | same policy per packaging format; dialog instead of popover; download from the releases page |
| Problem reports | `core/reporting`, `features/reporting` | `reporting` + `src/lib/reporting/` | no `faulthandler`/crash zip: logs are the current session's warnings; see gaps |
| Donation and reminder | `features/donation` | — | removed: FastZap does not ask for donations |
| Onboarding | `features/initial_setup` | `src/lib/onboarding/` | the language is applied on completion; the Flatpak section shows the `flatpak override` command with no link to Flatseal |
| Performance | `core/config/settings/performance.py` | `settings::performance` | see gaps |
| Packaging | `.github/packaging`, `tools/com.rtosta.zapzap.yaml`, `share/` | `packaging/`, `.github/workflows/build-*.yml` | deb, rpm, AppImage, Flatpak (GNOME 50), snap, Windows NSIS/MSI (x86_64; arm64 NSIS only), and universal dmg; no COPR, no Flathub (the manifest downloads dependencies over the network), and macOS without notarization |
| Evolution Go account (new) | — | `evolution` + `src/lib/evolution/` | native chat over an Evolution Go instance: pairing by QR, code, or passkey; text, media, replies, reactions, edits, deletion, receipts, and typing; see gaps |

## Known gaps

- **Chromium/QtWebEngine flags** (single process, process per site,
  predictable GC, JS heap limit, cache type/size, extra flags) have no
  equivalent in WebKitGTK/WebView2/WKWebView. The preferences are imported
  and preserved, but only `disable_gpu`/`software_rendering` (hardware
  acceleration policy), `persistent_cookies`, and `disable_animations` take
  effect.
- **WhatsApp Web session**: QtWebEngine profiles are not migratable; after
  the import each account must scan the QR code again.
- **`.bdic` dictionaries**: Chromium-only format. FastZap uses Hunspell via
  Enchant. The catalog is embedded (no `manifest.json`/ETag), and importing
  replaces the switch dialog with a conflict error; folder import does not
  exist. Windows and macOS use the system spell checker with no in-app
  language selection; the context menu is also only adjusted on Linux.
- **Notifications on Windows/macOS** do not report clicks to the app
  (`tauri-plugin-notification`); clicking does not open the conversation. The
  plugin also returns no identifiers: notifications with the same tag are not
  replaced, are not withdrawn when the conversation is read or on exit (the
  ZapZap macOS backend withdrew them). The portal does not receive the silent
  sound request.
- **Strict proxy** disables WebRTC instead of forcing UDP through the proxy:
  WebKit has no `disable_non_proxied_udp` policy like Chromium's. On Linux,
  WebKitGTK WebRTC is turned off natively; on other platforms the API is
  removed from every frame by script.
- **Mute** only takes effect on Linux (WebKitGTK); WebView2/WKWebView expose
  no muting through Tauri.
- **Audio/video calls** remain unavailable on WhatsApp Web, as in ZapZap.
- **Proxy authentication**: webviews do not accept proxy user/password; the
  password stays in the keyring, but the UI warns that credentials are not
  applied.
- **`dont_use_native_dialog`** is imported and shown, but FastZap always
  uses the system dialogs.
- **Downloads**: Tauri exposes no progress, pause, resume, or cancellation.
  The file lands in a temporary folder, and the download dialog, the
  multiple-downloads question, and "Save as" appear when it finishes; there
  is no 6-active-download queue, progress ring, or popover (the sidebar button
  opens the history). The MIME type reported by the server is unknown, so
  automatic opening checks content and extension only.
- **Permissions on Windows/macOS**: Tauri does not expose the
  WebView2/WKWebView permission prompt; the system's own prompts apply and
  the auto-grant options have no effect.
- **Reports**: no `faulthandler.log`, crash zips, or engine diagnostic trace.
  A panic becomes a report (the release build aborts right after); an
  unclean exit only records the marker, with no signal or previous-session
  logs. GPU/VAAPI/Vulkan diagnostics are not collected.
- **Evolution Go accounts** reproduce the WhatsApp Web layout (list with
  search, All/Unread/Groups filters, and Archived folder; conversation with
  wallpaper, tailed bubbles, ticks, and reactions; composer with attachment
  and emoji), but not the status sidebar, channels and calls, or audio
  recording. Pinning, archiving, unarchiving, and muting chats from FastZap
  do not exist (the "Archived" folder only mirrors the phone, with up to
  5 min of delay), nor do status, calls, voting in polls (FastZap shows the
  question and options, without votes), sent stickers, mentions, forwarding,
  and group creation. Business-message buttons only act when they are links;
  quick replies, lists, and copy codes stay as labels. Albums appear as
  separate photos, not in a grid. Animated Lottie stickers, comments, hidden
  messages from linked devices, and history notices remain an unsupported
  message, and video that WhatsApp serves only via static URL (official
  account ads) only opens on the phone. Replies to messages FastZap did not
  store show only the copy the server delivered (no thumbnail); only one's
  own messages can be deleted or edited; failed media sends have no "send
  again"; old media that expired on the WhatsApp CDN is unrecoverable (the
  server has no *media retry*). Old history depends on the pairing's initial
  sync or a request to the phone ("load older messages"); the initial sync
  only arrives once, and a conversation with no activity since then only
  appears after a fresh pairing of the instance. FastZap only receives that
  sync if, at pairing time, the server was already recording the journal
  (`PRIVATE_CHAT_HISTORY`) or FastZap's socket was open with `HISTORY_SYNC`;
  an instance paired before that must be paired again to bring the old
  conversations. Without `PRIVATE_CHAT_HISTORY` on the server, events lost
  while the WebSocket is down never come back, and a socket the server
  stopped feeding is only replaced on the next connection error; with the
  journal, missed messages appear within 30 s. Zoom, customizations, spell
  checker, and the built-in account switcher apply only to WhatsApp Web
  accounts; "send message to a number" opens WhatsApp Web, while the native
  chat has its own number field. The server admin key stays in the local
  keyring, so the mode suits personal use on one's own server.
