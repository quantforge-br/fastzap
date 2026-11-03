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
- Start with the system: the Background portal in Flatpak, an XDG autostart
  entry on other Linux installs and tauri-plugin-autostart on Windows and
  macOS, always with --hideStart.
- Auto theme follows the XDG Settings portal color-scheme and its changes,
  falling back to the toolkit theme.
- Client-side title bar with ZapZap's button themes (default, Adwaita, Plasma,
  iOS, Windows), button side and visibility, drag, double-click maximize and
  resize edges.
- Quick access panel window with accounts, unread counts, new account, mute,
  downloads and settings, following the always, when hidden and on demand
  modes, staying on top and remembering or locking its position.
- Interface translations imported from ZapZap's po/ catalogs, with live
  language switching in the UI.
- Settings window with ZapZap's pages (appearance, notifications, permissions,
  system and startup, language and downloads, network and privacy,
  performance, about), a restart bar for startup-only options, restore
  defaults and proxy password storage in the keyring.
- Optional menu bar with ZapZap's File, View, Chat, Users and Help menus as
  native popups, and Ctrl+Shift+I to open the web inspector of the active
  account.
- Interface scale applied to the shell and to every account zoom; WebKitGTK
  hardware acceleration, smooth scrolling and disable animations performance
  options.
- Accounts settings page with ZapZap's account cards (enable/disable, do not
  disturb, remove with confirmation), the edit dialog (name, generated or
  photo icon, profile photo sync, User-Agent) and the sidebar account context
  menu; New account (Ctrl+U) creates an unnamed account with a generated icon.
- Downloads following ZapZap's rules: Download dialog (Save, Open, Save as,
  Open folder, Cancel), automatic and ask every time behaviors, the multiple-
  download question with Allow once/Always allow/Block, ZapZap's file name
  sanitizing with a Chromium-style ' (N)' suffix for taken names, per-account
  Save As folder memory, auto-open of PDFs and images whose content matches
  the extension, and a history of 100 files.
- Downloads view (Ctrl+J, sidebar button, floating panel) with open, show in
  folder, delete file, remove from history and clear history, plus a busy
  indicator and a ✓ badge when a file is saved.
- Web permission requests on Linux follow ZapZap: notifications are always
  granted, features set to auto grant are allowed, anything else asks
  "Permission request" with No as the default, and a Yes is remembered for
  that account's page until it is recreated.
- Advanced customizations with ZapZap's folders and rules: global CSS/JS plus
  per-account files when the account does not inherit, @-moz-document
  userstyles filtered to WhatsApp Web, injection after every page load, and a
  settings page to create, edit, import (file or HTTPS URL up to 2 MB),
  duplicate, delete, enable and preview files; CSS changes refresh open pages.
- Spell checker on Linux with Hunspell dictionaries through Enchant: a catalog
  pinned to a wooorm/dictionaries commit with git blob SHA-1 verification,
  one-time install for the system locale, the language picker (up to 10
  languages, 4 recent), the dictionary manager with install, removal and .dic
  import, and system dictionaries listed read-only.
- Page context menu on Linux without navigation, link-opening and inspector
  entries, with Check Spelling and Languages… items.
- scripts/hunspell-catalog.mjs regenerates the embedded Hunspell catalog from
  a wooorm/dictionaries commit.
- Problem reports kept on the device: the Report a problem settings page, a
  form with a sanitized review, Markdown copied to the clipboard and the
  GitHub new-issue page, local records (20 files, 30 days), unclean-exit
  detection through a session marker, panic capture and the startup review
  prompt when failure notifications are on.
- Reset settings in the Report a problem page's maintenance section.
- Release check for official DEB, macOS and Windows builds: one GitHub request
  per process, a sidebar button with the update dialog and a row on the About
  page.
- Send message to number dialog (Ctrl+M) with country code picker, live
  validation matching the backend rules and an optional message.
- Keyboard shortcuts dialog listing the WhatsApp Web and FastZap shortcuts.
- Donation page and startup support reminder; the methods are ZapZap's
  (FastZap is a port) and the backend opens only their fixed URLs.
- First-run setup with six steps (basics, notifications, background, files,
  permissions, finish); Skip only marks it completed and Finish saves every
  choice, forcing Quit on close without a tray icon.
- Evolution Go REST client with separate admin and instance scopes, merged
  connect configuration, sanitized errors and ambiguous-send detection.
- Evolution Go event parser for messages, replies, media, reactions, edits,
  revokes, receipts, typing, history sync and connection events.
- Per-account SQLite chat store for Evolution Go accounts with deduplication,
  outbox states, receipts, reactions, edits, revokes, journal cursor and
  history-sync anchors.
- Evolution Go WebSocket loop with 20-second pings, bounded reconnect backoff,
  double-decoded frames, instance filtering and instance-token redaction.
- Evolution Go session manager: instance connect with preserved configuration,
  WebSocket events, journal catch-up from a saved cursor, unread counts,
  native notifications and notification clicks that open the chat.
- Evolution Go commands: account setup with keyring secrets (existing or new
  instance), pairing by QR or phone code, chats and paged messages, text and
  file sending through a local outbox, retry of failed sends, reactions,
  edits, deletes, read receipts, typing, cached media, older history requests,
  avatars and contacts.
- Typed IPC wrappers and formatting helpers for the Evolution Go chat.
- Evolution Go account setup dialog in Settings → Accounts: server address,
  admin key, instance listing, reuse or creation of an instance, and
  reconfiguration of existing accounts.
- Native Evolution Go chat in place of the webview: pairing by QR, phone code
  or passkey; connection state; chat list with avatars, search, unread counts
  and new chats by number; conversations with day separators, older pages,
  phone history requests, media, replies, reactions, edits, deletes, retries,
  receipts and typing; notification clicks open the chat.
- Linux packaging metadata: deb/rpm desktop entry template (whatsapp: and
  fastzap: links), an app-ID .desktop file and AppStream metainfo in
  packaging/share, and bundle metadata (publisher, license, descriptions,
  NSIS/WiX languages, ad-hoc macOS signing) in tauri.conf.json.
- packaging/common/build-info.sh marks official builds (FASTZAP_BUILD_* for
  build_info) and version.sh prints the version and checks that package.json,
  Cargo.toml and tauri.conf.json agree.
- Snap recipe (core24, gnome extension, WebKitGTK layout, D-Bus slot for the
  single-instance name, Secret Service plug) in packaging/snap, copied to
  snap/ by prepare.sh.

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
- The window starts with the configured theme instead of the toolkit default.
- Second launches arriving before startup finished are queued instead of
  crashing, and are handled off the Windows message callback to avoid a
  WebView2 deadlock.
- Turning start with the system off through --setSettings on launch removes
  the autostart entry.
- Autostart entries escape % and refuse paths with line breaks, and quick
  toggles of start with the system can no longer finish out of order.
- Notification photos are decoded with size and memory limits, off the async
  workers and two at a time.
- A notification closed by the page while it was still being posted is
  withdrawn instead of staying on screen, notification ids no longer collide
  after a page reload, the activation token is no longer lost when D-Bus
  signals interleave, and clicks on other applications' notifications are
  ignored.
- A link forwarded while its account page finished loading is no longer lost.
- macOS opens whatsapp: links delivered as Apple events.
- A locked quick access panel keeps its new placement instead of snapping back
  to the previous anchor.
- Windows and macOS builds no longer fail clippy on Linux-only notification
  helpers.
- Windows shows the unread count as a taskbar overlay icon instead of an
  unsupported badge.
- The tray Accounts action opens the account grid, as in ZapZap, instead of an
  empty panel.
- The npm Tauri packages are pinned to the Rust crates' minor versions so
  tauri dev no longer refuses to start.
- Screen sharing requests are no longer treated as camera requests on Linux,
  and permission prompts of a recreated account page are denied instead of
  granting the new page.
- Problem reports also remove quoted and unquoted credential assignments
  (passwords, tokens, API keys, Basic/Bearer authorization).
- A failed download no longer fails another account's transfer, and concurrent
  automatic downloads with the same name get distinct suffixes instead of
  replacing each other.
- Concurrent settings writes (downloads, dictionary provisioning, the UI) are
  serialized, so none of them loses another's change or shares the temporary
  file.
- An imported dictionary shows up in the open dictionary manager right away.
- Saving, importing, duplicating or deleting a CSS file restyles the open
  pages immediately.
- Reporting a problem stops with a warning when the clipboard cannot be
  written, instead of opening GitHub with nothing to paste; the report is
  marked copied only after the copy succeeds.
- App shortcuts (Ctrl+M, Ctrl+J, Ctrl+P and the rest) also work while the
  settings, downloads or another local screen has focus.
- Evolution Go QR and phone pairing requests are refused while WhatsApp is
  linked, since the server restarts a logged-in client to issue a new QR;
  passkey ceremonies open in the system browser.
- Evolution Go: the first journal read no longer sends an empty cursor, which
  the server rejected, so recovery now starts on a fresh install.
- Evolution Go: journal pages are applied in their original order in one
  transaction, so a read receipt no longer lands before its message and the
  cursor only advances when every event was stored.
- Evolution Go: a transient journal error (HTTP 503, network, timeout) is
  retried with backoff instead of turning recovery off for the session, and a
  backlog longer than one batch keeps being read.
- Evolution Go: a late send error no longer downgrades a message the server
  already confirmed as sent, delivered or read.
- Evolution Go: an instance that is linked but disconnected from WhatsApp is
  shown as offline instead of online, and Retry asks the server to reconnect
  it.
- Evolution Go: starting and stopping sessions is serialized, and stopping a
  session cancels its work in flight, so a deleted or reconfigured account no
  longer keeps an unreachable session or connects the instance afterwards.
- Evolution Go: local chats, media and the journal cursor are tied to the
  server, instance and linked phone; pointing an account elsewhere or linking
  another phone starts from a clean store instead of mixing conversations.
- Evolution Go: the WebSocket reconnects when nothing (not even a pong)
  arrives for 65 seconds, and its writes and close are time-limited, so a
  silently dead network no longer looks online.
- Evolution Go: an open conversation is no longer reloaded (clearing the draft
  and sending another read request) every time the chat list refreshes, and an
  older page that arrives after switching chats is discarded instead of shown
  in the new chat.
- The single-instance D-Bus name stays
  io.github.quantmind_br.fastzap.SingleInstance after the bundle identifier
  change, so Flatpak still lets FastZap own it.

### Changed

- docs/parity.md declares that Windows and macOS notifications are not
  replaced, withdrawn or removed on exit.
- Account icons use ZapZap's generated SVG icon and its icon profile format,
  which keeps both the generated icon and a photo; a synced profile photo no
  longer discards the generated icon. The default account (id 1) cannot be
  removed.
- About's Report a problem row opens the report dialog instead of the issue
  list.
- The main window shows the onboarding on first run, then the support
  reminder, and opens the send message, shortcuts and donation panels instead
  of placeholders; architecture and parity docs cover them.
- Architecture, parity and testing documentation describe the Evolution Go
  account mode, its secrets handling and its known gaps.
- Docs: architecture and testing notes describe the Evolution Go journal
  retries, store binding, lifecycle lock and WebSocket deadlines.
- App icons (Linux PNGs, Windows .ico, macOS .icns, Store logos) are generated
  from the FastZap SVG in packaging/share/icons instead of the Tauri template.
- Tauri's bundle identifier is io.github.quantmind-br.fastzap, because bundle
  identifiers cannot contain an underscore; the app ID, D-Bus name, keyring
  service and config, data and cache folders stay
  io.github.quantmind_br.fastzap. Saved window geometry starts over once.
- Windows builds are labelled "Windows x86_64" and "Windows arm64" (one build
  makes both the NSIS and MSI installers), and both keep the release check.
- pnpm is pinned through packageManager (pnpm@12.9.1) and the lockfile records
  it, so CI, Flatpak and snap builds use the same version.
