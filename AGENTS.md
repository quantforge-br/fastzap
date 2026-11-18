# Repository Guidelines

## Project Overview

FastZap is a multi-account WhatsApp desktop client (Tauri 2 + Rust +
SvelteKit/Svelte 5), a rewrite of ZapZap 7.5.2 (local reference at
`~/dev/zapzap`). There are two account kinds: **WhatsApp Web** (one isolated
native webview per account) and **Evolution Go** (native Svelte chat over the
Evolution Go server's REST API + WebSocket). GPL-3.0-or-later license.

Before changing anything, read `CHANGELOG.md`, `docs/architecture.md`,
`docs/maintenance.md`, `docs/COMMIT_CONVENTION.md`, `docs/parity.md`, and the
relevant section of `docs/testing.md`. Docs, code, comments, `CHANGELOG.md`,
and commits are in English.

## Architecture & Data Flow

- **Rust owns the state**: preferences, accounts, secrets, and native
  integrations. The frontend only renders and invokes commands.
- **Layers** (`docs/architecture.md` §Overview): `settings`, `secrets`,
  `paths`, and `i18n` do not depend on any feature module. Features do not
  depend on each other; they coordinate through `app::state::AppState` or via
  events. Only `src-tauri/src/commands/` knows the IPC call format.
- **Windows and webviews** (`tauri.conf.json` has `app.windows: []`; everything
  is created in Rust):
  - `main` window with the `ui` child webview (SPA, `/` route) —
    `app/window.rs`;
  - `panel` window (`/panel` route, quick-access panel) — `app/panel.rs`;
  - one `wa-<id>` child webview per enabled WhatsApp Web account —
    `webview/manager.rs`; data in `<data>/profiles/<id>` (macOS:
    `data_store_identifier`); on Linux the children live in a `GtkOverlay`
    (`webview/stage.rs`);
  - `popup-N` popups with no IPC.
- **Bootstrap** (`lib.rs::run`): logger and panic hook → plugins
  (`single_instance` first) → `.manage(...)` → `setup` with
  `app/bootstrap.rs::init_state` (builds `AppState`, runs the ZapZap
  import), window, webviews, Evolution sessions, tray, panel, deep links,
  update check.
- **Backend → UI**: `domain://event` events with **full snapshots**
  (not deltas): `settings://changed`, `accounts://changed`,
  `layout://changed`, `unread://changed`, `ui://action`,
  `downloads://{changed,prompt,saved}`, `permissions://request`,
  `spellcheck://changed`, `update://available`, `evolution://event`.
  Constants in `app/events.rs` and the domain modules; mirrored by hand in
  `events` in `src/lib/ipc/index.ts`.
- **Evolution Go**: WebSocket (`evolution/ws.rs`) or `/chat/events` journal
  (resumed from the cursor after each reconnection) →
  `evolution/model.rs::parse` → `ChatStore` (SQLite WAL at
  `<data>/evolution/<id>/chats.db`) → `evolution://event` (`UiEvent` with
  `type`) → the UI re-reads via `evo_chats`/`evo_messages`.
- **Remote bridge**: `wa-*` webviews at `https://web.whatsapp.com/*` only invoke
  the `wa_*` commands in `commands/bridge.rs`
  (`capabilities/whatsapp-remote.json`). The calling account comes **from the
  webview label, never from the arguments**; `wa_shortcut` only accepts
  `REMOTE_ACTIONS`.

## Key Directories

| Path | Role |
|---|---|
| `src-tauri/src/app/` | lifecycle, windows, panel, events, theme, CLI, D-Bus (Linux), `actions.rs` (shortcut/action target) |
| `src-tauri/src/commands/` | IPC surface, one file per domain |
| `src-tauri/src/webview/` | WhatsApp Web webviews, navigation allowlist, injected scripts (`scripts/*.js`), deep links, unread counts |
| `src-tauri/src/evolution/` | `client.rs` (REST), `ws.rs`, `store.rs` (SQLite), `model.rs` (parser), `mod.rs` (sessions) |
| `src-tauri/src/settings/` | typed `Settings` + `store.rs` (versioned JSON) |
| `src-tauri/src/migration/` | one-shot ZapZap import (`ZapZap.conf`, `zapzap.db`, customizations) |
| `src-tauri/src/accounts/` | `accounts.db`, icons, avatars, User-Agent presets |
| `src/routes/` | `+page.svelte` (main shell), `panel/+page.svelte` |
| `src/lib/ipc/` | `index.ts` (`invoke` wrappers) and `generated/` (versioned ts-rs types) |
| `src/lib/components/` | reusable components (`settings/Row`, `SwitchRow`, `SelectRow`, `RestartBar`, `ConfirmDialog`, `MenuBar`) |
| `src/lib/evolution/` | native chat: `ChatView`, `Conversation`, `PairingPanel`, `SetupDialog`, `format.ts` |
| `src/lib/i18n/` | `index.svelte.ts` (`t`, `tn`), `plural.ts`, `catalogs/*.json` (generated) |
| `po/` | translations (`*.po`, ZapZap origin); `LINGUAS` is informational only |
| `packaging/` | `common/*.sh`, deb/rpm template, Flatpak (+ `shared-modules` submodule), snap, metainfo/desktop/icons |
| `.github/workflows/` | `quality.yml`, `build-*.yml`, `release-candidate.yml`, `release-deploy.yml` |

## Development Commands

```bash
pnpm install && git submodule update --init
pnpm tauri dev                       # Vite at 127.0.0.1:1420 (strictPort) + app
RUST_LOG=debug pnpm tauri dev        # env_logger; default info
pnpm build                           # static SPA in build/ (required by the Rust build)
pnpm tauri build --bundles deb,rpm,appimage

pnpm check                           # svelte-kit sync + svelte-check
pnpm test                            # Vitest
pnpm test src/lib/chat/validate.test.ts
pnpm test -t "mirrors the backend rules"

cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml [evolution::client]
(cd src-tauri && cargo test)         # regenerates src/lib/ipc/generated/ (see below)
git diff --check

pnpm i18n                            # po/*.po -> src/lib/i18n/catalogs/*.json
node scripts/hunspell-catalog.mjs <sha-40-hex>   # only to rotate the dictionary pin
packaging/common/version.sh --check  # versions + metainfo <release>
```

## Code Conventions & Common Patterns

### Mandatory rules

- **Persistence**: preserve keys and values. A `settings.json` format change =
  bump `SETTINGS_VERSION` (`settings/mod.rs`) and add a step in
  `settings::store::migrate` (currently empty); never change the meaning of a
  field. Defaults and names mirror ZapZap; when changing a key, update
  `src-tauri/src/migration/`.
- **Secrets** (proxy password, `AdminKey`, `InstanceToken`) only in the keyring
  (`secrets.rs`, `SecretStore` trait); never in JSON, logs, events, or error
  messages. No command returns the admin key. The WebSocket URL contains the
  admin key: never log it.
- **ACL**: do not widen `capabilities/whatsapp-remote.json` without justifying
  it in `CHANGELOG.md`.
- **Real Evolution Go** (create instance, connect, pair, send) only with the
  user's explicit authorization; tests use mock servers.
- Persisted IDs kept separate from translated labels (`settings/pages.ts`,
  `<select>` values).
- Structural changes require updating `docs/architecture.md` and
  `docs/parity.md`.
- Every change (code, docs, tests, deps, packaging, workflows) goes into the
  `In development` section of `CHANGELOG.md` under `Added`/`Changed`/`Fixed`/
  `Removed`: English, present tense describing the result, ~80 columns.
- Identical version in `package.json`, `src-tauri/Cargo.toml`,
  `src-tauri/tauri.conf.json`, and the metainfo's first `<release>`.
- Commits: `<type>(<scope>): <imperative description>` in English
  (`docs/COMMIT_CONVENTION.md`); usual scopes `evolution`, `webview`, `app`,
  `settings`, `accounts`, `packaging`. Always deliver a suggested message.

### New IPC command (all steps)

1. `#[tauri::command]` in `commands/<domain>.rs`, returning
   `CommandResult<T>`; `async` if it creates a webview or opens a dialog
   (deadlock on WebView2).
2. Register in `generate_handler!` (`lib.rs`).
3. Name in `UI_COMMANDS` or `BRIDGE_COMMANDS` (`build.rs`) → generates
   `permissions/autogenerated/<name>.toml`.
4. Grant `allow-<kebab-name>` in `capabilities/default.json` (`ui`,
   `panel`) or `whatsapp-remote.json`.
5. Types with `#[derive(TS)] #[ts(export)]`; regenerate with
   `(cd src-tauri && cargo test)`.
6. Wrapper in `src/lib/ipc/index.ts`, e.g.:
   `chats: (accountId: number) => invoke<ChatSummary[]>("evo_chats", { accountId })`.
   Top-level arguments in camelCase in JS; struct fields in snake_case.

### Rust

- snake_case command names with a domain prefix: `evo_`, `wa_`, `spell_`,
  `report_`, `custom_`, `accounts_`, `layout_`, `settings_`, `app_`.
- Errors: `thiserror` per domain (`EvoError`, `StoreError`, `AccountError`…),
  `anyhow` only in orchestration. `CommandError { message }` has
  `From<E: Display>`, so `?` works in commands. Server messages are
  sanitized; `Secret` prints `<redacted>` in `Debug`.
- Async: `tauri::async_runtime::spawn`; `std::sync::{Mutex,RwLock}` for short
  sections, `tokio::sync::Mutex` when the lock is held across `.await`.
- Logs: `log` macros (`warn!`, `info!`), lowercase messages with `{e}`;
  no `println!`. Release uses `panic = "abort"`.
- Persisted enums in snake_case, **except `ProxyKind`** (`"NoProxy"`,
  `"Socks5Proxy"`…). Do not normalize.
- Options read at webview creation (proxy, strict WebRTC, engine) require
  `recreate_all`; in the UI they use `RestartBar` when they only apply at the
  next start.
- Evolution: `AdminKey` ≠ `InstanceToken` (distinct types, `apikey` header);
  base URL `https` (or `http` on loopback); an expired send becomes `Ambiguous`
  and is never retried automatically; `GET /instance/qr` only with
  `LoggedIn == false`.
- Per-platform `cfg`: D-Bus, GTK stage, portal, WebKit permissions, and
  freedesktop notifications are Linux-only; autostart plugin off Linux.

### Svelte / TypeScript

- Svelte 5 runes only: inline-typed `$props()`, `$state`, `$derived`,
  `$effect`, `{@render}`/snippets, `onclose`/`onconfirm` callbacks. No
  `export let`, `on:`, `<slot>`, or `svelte/store`.
- Module state in `*.svelte.ts` (`stores/settings.svelte.ts`:
  `startSettings()`, `patchSettings(patch)`). Events replace the whole state.
- Initial prop snapshots become local state with `untrack(() => prop)`
  (`AccountEditDialog`, `ReportDialog`, `LanguagePicker`). In `$effect`,
  incidental reads (called functions reading another `$state`) also go in
  `untrack`, or they become dependencies.
- Events: `on<T>(events.x, handler)` returns `Promise<UnlistenFn>`; cleanup
  `return () => unlisten.then((f) => f())`.
- `invoke` errors arrive as a `{ message }` object, not an `Error`: read
  `.message`, not `String(e)`.
- Dialogs and screens covering the content call `layout.setOverlay(true)`
  (native webviews stay above the HTML); menus are native (`MenuBar`), not
  HTML.
- Style: per-component scoped `<style>`; global tokens in `src/app.css`
  (`--accent`, `--bg`, `--fg`, `--danger`) and `.btn`, `.muted`,
  `.info-box` classes.
- Names: `PascalCase.svelte` components; lowercase/camelCase `.ts` modules;
  pure logic in a `.ts` file next to the component, with a colocated
  `*.test.ts`.
- i18n: `t("English text", { name })`, `tn(singular, plural, n)`; Rust uses
  `i18n::tr("…")`. `msgid` is the English text and must say "FastZap" (the
  script renames ZapZap); `msgctxt` is discarded. Static tables call `t()` at
  render time. There is no extractor: new strings stay in English until a
  `.po` entry exists.
- `patchSettings` accepts `unknown` and the backend ignores unknown keys:
  a mistyped key is a silent no-op. Double-check the Rust field.
- Shortcuts live in several places. `shortcutFor` in
  `webview/scripts/bridge.js` and `src/lib/shell/shortcuts.ts` are identical
  maps, compared by `src/lib/shell/shortcuts.test.ts`; a test in
  `commands/bridge.rs` requires every `shortcutFor` action in `bridge.js` to
  be in `REMOTE_ACTIONS`. Exceptions: `Ctrl+Shift+V` (`paste_plain_text`)
  only exists in the page, and the `e.repeat` filter only in `+page.svelte`.
  Still synced by hand: `MenuBar.svelte` accelerators,
  `shortcuts/ShortcutsDialog.svelte`, and `app/actions.rs::dispatch_for`.

## Important Files

- Entries: `src-tauri/src/main.rs` → `lib.rs::run()`; `src/routes/+layout.ts`
  (`ssr = false`), `+layout.svelte`, `+page.svelte`, `panel/+page.svelte`.
- `src-tauri/build.rs`: `UI_COMMANDS`/`BRIDGE_COMMANDS` lists (ACL manifest)
  and embeds `src/lib/i18n/catalogs/*.json` (missing directory = zero
  languages, no error).
- `src-tauri/tauri.conf.json`: identifier `io.github.quantmind-br.fastzap`
  (hyphen); the runtime `APP_ID` (`lib.rs`, folders, D-Bus, keyring, Flatpak)
  is `io.github.quantmind_br.fastzap` (underscore). IDs use `quantmind`, URLs
  and the official repository use `quantforge-br/fastzap`. **Do not unify** —
  it would move users' data folders.
- `src-tauri/.cargo/config.toml`: `TS_RS_EXPORT_DIR = ../src/lib/ipc/generated`.
- `src-tauri/src/app/state.rs` (`AppState`), `commands/mod.rs`
  (`CommandError`), `commands/bridge.rs`, `webview/navigation.rs`
  (allowlist), `settings/store.rs`, `secrets.rs`, `build_info.rs`
  (`FASTZAP_BUILD_*`), `update.rs`.
- `src-tauri/resources/hunspell-catalog.json` (generated, versioned, via
  `include_str!`).
- `packaging/common/version.sh`, `build-info.sh`, `release-upload.sh`;
  `packaging/flatpak/io.github.quantmind_br.fastzap.yml`;
  `packaging/share/metainfo/io.github.quantmind_br.fastzap.metainfo.xml`.

## Runtime/Tooling Preferences

- **pnpm** pinned in `packageManager` (`pnpm@12.9.1`); CI uses Node 24. Do not
  use npm/yarn or touch the lockfile without need.
- Rust edition 2024, `rust-version = 1.85`, stable toolchain in CI.
- Linux needs `libwebkit2gtk-4.1-dev` (WebKitGTK ≥ 2.40),
  `libayatana-appindicator3-dev`, `librsvg2-dev`, `libssl-dev`,
  `libdbus-1-dev`.
- The Rust build embeds `../build`: run `pnpm build` before `cargo` on a fresh
  clone (CI does this).
- **ts-rs bindings**: the `src-tauri/.cargo/` config only applies with the cwd
  in `src-tauri/`. `cargo test --manifest-path src-tauri/Cargo.toml` from the
  root exports to `src-tauri/bindings/` (ignored) and does **not** update
  `src/lib/ipc/generated/`. Changed an exported type? Run
  `(cd src-tauri && cargo test)` and commit `generated/`. The CI `rust` job
  (Linux) fails when `src/lib/ipc/generated/` or
  `src-tauri/permissions/autogenerated/` diverge after
  `(cd src-tauri && cargo test --locked)`.
- No other worktree may point `CARGO_TARGET_DIR` at this checkout's
  `src-tauri/target`: Cargo reuses the build script compiled by it
  (`UI_COMMANDS`/`BRIDGE_COMMANDS` lists and catalog path). `make install`
  runs `touch src-tauri/build.rs` before building.
- Generated and versioned files: `src/lib/ipc/generated/`,
  `src/lib/i18n/catalogs/` (do not edit by hand; `pnpm i18n` wipes and
  recreates), `src-tauri/permissions/autogenerated/`. Ignored: `build/`,
  `.svelte-kit/`, `src-tauri/gen/schemas`, `src-tauri/bindings/`,
  `packaging/common/build-info.env`.

## Testing & QA

- **Vitest** (`vite.config.js`: `src/**/*.test.ts`, Node environment, no DOM
  and no Tauri mocks): tests pure functions in `src/lib/`. Style
  `describe("<module>")` + `it("<behavior sentence>")`. Svelte components
  have no tests.
- **Rust**: `#[cfg(test)] mod tests` at the end of the file; names describe the
  behavior (`base_urls_require_https_except_loopback`); async with
  `#[tokio::test]`. Dev-deps: `wiremock`, `tempfile`.
  - Evolution REST with `wiremock::MockServer` (checks the `apikey` header per
    scope); WebSocket against `tokio-tungstenite` at `127.0.0.1:0`.
  - Secrets with `secrets::MemoryStore`; SQLite with
    `AccountStore::in_memory()`/`ChatStore::in_memory()`; files in
    `tempfile::tempdir()`. Do not mutate environment variables in tests.
- **CI** (`quality.yml`, pushes to `main` and PRs): `rust` (Ubuntu/Windows/
  macOS: `pnpm build`, fmt, clippy `--locked -D warnings`,
  `cargo test --locked` with cwd in `src-tauri/`; on Linux, fails if
  `src/lib/ipc/generated/` or `src-tauri/permissions/autogenerated/` diverge
  from the committed files), `frontend` (`pnpm i18n` and fails if
  `src/lib/i18n/catalogs/` diverges, `pnpm check`, `pnpm test`), `packaging`
  (`version.sh --check`, `desktop-file-validate`,
  `appstreamcli validate --no-net`, `shellcheck`, `actionlint`). No coverage
  target; `git diff --check` local only.
- **Manual** (`docs/testing.md` §Manual checks): child webviews, focus, tray,
  notifications, and the composer require `pnpm tauri dev` with two WhatsApp
  Web accounts, deep link `xdg-open "whatsapp://send?phone=..."`, downloads,
  theme, and grid. No automated test: 65 s WebSocket watchdog, injected scripts
  in `webview/scripts/` (only the `shortcutFor` map in `bridge.js` is tested).
- Before delivering: `pnpm check`, `pnpm test`, `cargo fmt … --check`,
  `cargo clippy … -D warnings`, `cargo test …`, `git diff --check`.
