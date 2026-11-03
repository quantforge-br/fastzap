# Changelog

## 0.1.0 — In development

### Added

- Tauri v2 + SvelteKit project scaffold, GPL-3.0 license, maintenance guide,
  commit convention, architecture, parity and testing documentation.
- Quality workflow running Rust formatting, clippy, tests, `svelte-check` and
  Vitest.
- Typed settings domains mirroring ZapZap's names and defaults, stored in a
  versioned `settings.json` with atomic writes and JSON merge-patch updates.
- System keyring storage for secrets (proxy password, Evolution Go keys).
- SQLite account store with WhatsApp Web and Evolution Go account kinds,
  per-account User-Agent, zoom, notifications and ordering.
- One-shot import of an existing ZapZap installation (native or Flatpak):
  `ZapZap.conf` preferences including legacy keys, accounts from `zapzap.db`,
  customization files and the proxy password.
- TypeScript bindings generated from Rust types with ts-rs.
- WhatsApp Web shell: one isolated child webview per enabled account with
  per-account User-Agent, zoom and storage, navigation allowlist, external
  links in the system browser, grid view and active-account switching.
- Injected bridge for unread counts from the page title, app shortcuts inside
  WhatsApp Web, plain-text paste (including spreadsheet tables), theme
  controller, account selector, profile photo sync and the ported ZapZap
  page scripts (Ctrl+arrow fix, PDF click, resizable chat list, WebRTC shield).
- Open chat by number and `whatsapp:`/wa.me deep link helpers.
- Command ACL: local UI and WhatsApp Web capabilities with an explicit
  per-command allowlist.
- Linux overlay stage so child webviews can be positioned under Wayland.
- Minimal Svelte shell: account sidebar with avatars, unread badges and
  do-not-disturb marker, grid and settings buttons.
- System tray with ZapZap's three icon themes and red unread counter, the
  Show/Accounts/monitoring panel/Mute/Settings/Support/Quit menu, launcher
  badge, and audio muting of WhatsApp Web pages (Linux).
- Native notifications for WhatsApp Web messages: XDG portal in Flatpak,
  `org.freedesktop.Notifications` elsewhere on Linux and the system
  notification center on Windows/macOS, with ZapZap's privacy options, per
  account do-not-disturb, silent mode, circular sender photos, tag
  replacement, closing when read and click-to-open-chat with activation
  tokens.
- Window lifecycle: closing hides FastZap to the tray unless quit on close is
  set, an optional confirmation with "Don't ask again" also guards Quit, the
  window starts hidden with start in background or --hideStart once setup is
  done, and its size, position and maximized state are restored.
- Command-line options --setSettings KEY VALUE (ZapZap key names), --wayland
  and --hideStart, plus the display backend preference applied through
  GDK_BACKEND.
- A second launch forwards its arguments: settings are applied,
  fastzap://restart restarts the app and the first whatsapp:/wa.me link opens
  in the active account; links passed at startup open once the page loads.
  SIGTERM and SIGINT quit normally and remove shown notifications.
- Flatpak builds own the application ID on D-Bus and implement
  org.freedesktop.Application, so the desktop activates the running instance,
  opens whatsapp: links and passes the focus token.

### Fixed

- Account updates and creation are atomic, a JSON `null` icon clears the
  avatar, partial reorders append the omitted accounts and imported accounts
  never replace existing ones.
- An unreadable `settings.json` stops startup instead of being replaced by
  defaults.
- ZapZap import decodes QSettings values like Qt: `@@` strings, quoted and
  escaped values, quoted `@Variant` lists and QVariant maps.
- An incomplete ZapZap import is retried on the next starts (up to three
  times) without touching preferences, existing secrets are kept, native
  installs are found through the XDG directories and CSS preview mappings
  are imported.
- A second launch focuses the running instance instead of starting another
  process that would race on settings and accounts.
- Plain-text paste no longer crashes on a clipboard HTML document ending in an
  unterminated tag and decodes every HTML entity in spreadsheet cells, as
  ZapZap does.
- Profile photo sync only fetches HTTPS images from WhatsApp's CDN without
  following redirects, reads at most 5 MiB, bounds the decoded image size and
  runs at most two downloads at a time.
- WhatsApp Web pages can only trigger listed app shortcuts; plain-text paste
  is handled natively on Linux and needs a focused window elsewhere, so page
  scripts cannot read the clipboard. Shortcuts act on the account that sent
  them, also in grid view.
- Account and settings commands that create webviews run asynchronously,
  avoiding the WebView2 deadlock of synchronous commands on Windows.
- macOS builds apply the configured proxy and require macOS 14, where each
  account gets its own WebKit data store instead of sharing the default
  session.
- Strict proxy mode disables WebRTC in every frame (natively on Linux) instead
  of requesting relay-only ICE, which pages could undo and which still
  bypassed the proxy.
- Disabling an account clears its unread count from the sidebar, tray,
  launcher badge and the other accounts' selector indicators.
- Changing the proxy, WebRTC shield, strict proxy or Ctrl+arrow fix recreates
  the account webviews, since a reload kept the settings captured when they
  were created.
- WhatsApp popups to allowed hosts open in a FastZap window that keeps the
  navigation allowlist and sends other links to the system browser, instead of
  an unrestricted default webview.
- The integrated account selector opens a native menu with the other enabled
  accounts and their unread counts; clicking one switches to it. The button
  did nothing before.
- Keyboard focus moves to FastZap's own screens while they cover the accounts
  and returns to the active account afterwards; keys used to go to the hidden
  WhatsApp page.
