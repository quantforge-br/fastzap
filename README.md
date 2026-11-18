# FastZap

Multi-account desktop client for WhatsApp written in Rust + Tauri v2, a
spiritual successor to [ZapZap](https://github.com/rafatosta/zapzap). Each account
can be one of two kinds:

- **WhatsApp Web** — web.whatsapp.com in a per-account isolated webview
  (own cookies, session, and cache);
- **Evolution Go** — login to an
  [Evolution Go](https://github.com/EvolutionAPI/evolution-go) instance, with
  QR or phone-code pairing and a native chat interface.

## Installation

Packages (deb, rpm, AppImage, Flatpak, snap, Windows installers, and macOS
dmg) are attached to GitHub releases. To build them locally, see
[`docs/maintenance.md`](docs/maintenance.md#packaging).

## Development

Requirements: stable Rust, Node 24+, pnpm (version in `packageManager`), and the Tauri
system dependencies
(on Linux: `webkit2gtk-4.1`, `libsoup-3.0`, `libappindicator`/`ayatana`).

```bash
git submodule update --init
pnpm install
pnpm tauri dev
```

Technical documentation in [`docs/`](docs/README.md).

## License

GPL-3.0-or-later. WhatsApp Web integration scripts were ported from
ZapZap, which uses the same license.
