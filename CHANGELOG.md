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
