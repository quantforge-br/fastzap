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
