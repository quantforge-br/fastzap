# Repository Guidelines

## Project Overview

FastZap é um cliente desktop multi-conta de WhatsApp (Tauri 2 + Rust +
SvelteKit/Svelte 5), reescrita do ZapZap 7.5.2 (referência local em
`~/dev/zapzap`). Há dois tipos de conta: **WhatsApp Web** (um webview nativo
isolado por conta) e **Evolution Go** (chat nativo em Svelte sobre a API REST +
WebSocket do servidor Evolution Go). Licença GPL-3.0-or-later.

Antes de alterar, leia `CHANGELOG.md`, `docs/architecture.md`,
`docs/maintenance.md`, `docs/COMMIT_CONVENTION.md`, `docs/parity.md` e a seção
pertinente de `docs/testing.md`. Os docs estão em pt-BR; código, comentários,
`CHANGELOG.md` e commits são em inglês.

## Architecture & Data Flow

- **Rust é dono do estado**: preferências, contas, segredos e integrações
  nativas. O frontend só renderiza e chama comandos.
- **Camadas** (`docs/architecture.md` §Visão geral): `settings`, `secrets`,
  `paths` e `i18n` não dependem de módulos de feature. Features não dependem
  entre si; coordenam-se por `app::state::AppState` ou por eventos. Só
  `src-tauri/src/commands/` conhece o formato das chamadas IPC.
- **Janelas e webviews** (`tauri.conf.json` tem `app.windows: []`; tudo é
  criado em Rust):
  - janela `main` com o webview filho `ui` (SPA, rota `/`) —
    `app/window.rs`;
  - janela `panel` (rota `/panel`, painel de acesso rápido) — `app/panel.rs`;
  - um webview filho `wa-<id>` por conta WhatsApp Web habilitada —
    `webview/manager.rs`; dados em `<data>/profiles/<id>` (macOS:
    `data_store_identifier`); no Linux os filhos ficam num `GtkOverlay`
    (`webview/stage.rs`);
  - popups `popup-N` sem IPC.
- **Bootstrap** (`lib.rs::run`): logger e panic hook → plugins
  (`single_instance` primeiro) → `.manage(...)` → `setup` com
  `app/bootstrap.rs::init_state` (monta `AppState`, roda a importação do
  ZapZap), janela, webviews, sessões Evolution, tray, painel, deep links,
  checagem de atualização.
- **Backend → UI**: eventos `domínio://evento` com **snapshots completos**
  (não deltas): `settings://changed`, `accounts://changed`,
  `layout://changed`, `unread://changed`, `ui://action`,
  `downloads://{changed,prompt,saved}`, `permissions://request`,
  `spellcheck://changed`, `update://available`, `evolution://event`.
  Constantes em `app/events.rs` e nos módulos de domínio; espelhadas à mão em
  `events` de `src/lib/ipc/index.ts`.
- **Evolution Go**: WebSocket (`evolution/ws.rs`) ou journal `/chat/events`
  (retomado do cursor após cada reconexão) → `evolution/model.rs::parse` →
  `ChatStore` (SQLite WAL em `<data>/evolution/<id>/chats.db`) →
  `evolution://event` (`UiEvent` com `type`) → a UI refaz a leitura via
  `evo_chats`/`evo_messages`.
- **Ponte remota**: webviews `wa-*` em `https://web.whatsapp.com/*` só chamam
  os comandos `wa_*` de `commands/bridge.rs`
  (`capabilities/whatsapp-remote.json`). A conta chamadora vem **do rótulo do
  webview, nunca dos argumentos**; `wa_shortcut` aceita só `REMOTE_ACTIONS`.

## Key Directories

| Caminho | Papel |
|---|---|
| `src-tauri/src/app/` | ciclo de vida, janelas, painel, eventos, tema, CLI, D-Bus (Linux), `actions.rs` (destino de atalhos/ações) |
| `src-tauri/src/commands/` | superfície IPC, um arquivo por domínio |
| `src-tauri/src/webview/` | webviews WhatsApp Web, allowlist de navegação, scripts injetados (`scripts/*.js`), deep links, não lidas |
| `src-tauri/src/evolution/` | `client.rs` (REST), `ws.rs`, `store.rs` (SQLite), `model.rs` (parser), `mod.rs` (sessões) |
| `src-tauri/src/settings/` | `Settings` tipado + `store.rs` (JSON versionado) |
| `src-tauri/src/migration/` | importação única do ZapZap (`ZapZap.conf`, `zapzap.db`, customizações) |
| `src-tauri/src/accounts/` | `accounts.db`, ícones, avatares, presets de User-Agent |
| `src/routes/` | `+page.svelte` (shell principal), `panel/+page.svelte` |
| `src/lib/ipc/` | `index.ts` (wrappers `invoke`) e `generated/` (tipos ts-rs, versionados) |
| `src/lib/components/` | componentes reutilizáveis (`settings/Row`, `SwitchRow`, `SelectRow`, `RestartBar`, `ConfirmDialog`, `MenuBar`) |
| `src/lib/evolution/` | chat nativo: `ChatView`, `Conversation`, `PairingPanel`, `SetupDialog`, `format.ts` |
| `src/lib/i18n/` | `index.svelte.ts` (`t`, `tn`), `plural.ts`, `catalogs/*.json` (gerados) |
| `po/` | traduções (`*.po`, origem ZapZap); `LINGUAS` é só informativo |
| `packaging/` | `common/*.sh`, template deb/rpm, Flatpak (+ submódulo `shared-modules`), snap, metainfo/desktop/ícones |
| `.github/workflows/` | `quality.yml`, `build-*.yml`, `release-candidate.yml`, `release-deploy.yml` |

## Development Commands

```bash
pnpm install && git submodule update --init
pnpm tauri dev                       # Vite em 127.0.0.1:1420 (strictPort) + app
RUST_LOG=debug pnpm tauri dev        # env_logger; padrão info
pnpm build                           # SPA estática em build/ (exigida pelo build Rust)
pnpm tauri build --bundles deb,rpm,appimage

pnpm check                           # svelte-kit sync + svelte-check
pnpm test                            # Vitest
pnpm test src/lib/chat/validate.test.ts
pnpm test -t "mirrors the backend rules"

cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml [evolution::client]
(cd src-tauri && cargo test)         # regenera src/lib/ipc/generated/ (ver abaixo)
git diff --check

pnpm i18n                            # po/*.po -> src/lib/i18n/catalogs/*.json
node scripts/hunspell-catalog.mjs <sha-40-hex>   # só para trocar o pin de dicionários
packaging/common/version.sh --check  # versões + <release> do metainfo
```

## Code Conventions & Common Patterns

### Regras obrigatórias

- **Persistência**: preserve chaves e valores. Mudança de formato do
  `settings.json` = incrementar `SETTINGS_VERSION` (`settings/mod.rs`) e
  adicionar uma etapa em `settings::store::migrate` (hoje vazio), nunca mudar o
  significado de um campo. Defaults e nomes espelham o ZapZap; ao mudar uma
  chave, atualize `src-tauri/src/migration/`.
- **Segredos** (senha do proxy, `AdminKey`, `InstanceToken`) só no keyring
  (`secrets.rs`, trait `SecretStore`); nunca em JSON, logs, eventos ou
  mensagens de erro. Nenhum comando devolve a chave admin. A URL do
  WebSocket contém a chave admin: não a registre em log.
- **ACL**: não amplie `capabilities/whatsapp-remote.json` sem justificar no
  `CHANGELOG.md`.
- **Evolution Go real** (criar instância, conectar, parear, enviar) só com
  autorização explícita do usuário; testes usam servidores mock.
- IDs persistidos separados de rótulos traduzidos (`settings/pages.ts`,
  valores de `<select>`).
- Mudança estrutural exige atualizar `docs/architecture.md` e
  `docs/parity.md`.
- Toda mudança (código, docs, testes, deps, empacotamento, workflows) entra
  na seção `In development` do `CHANGELOG.md` em `Added`/`Changed`/`Fixed`/
  `Removed`: inglês, tempo presente descrevendo o resultado, ~80 colunas.
- Versão idêntica em `package.json`, `src-tauri/Cargo.toml`,
  `src-tauri/tauri.conf.json` e no primeiro `<release>` do metainfo.
- Commits: `<type>(<scope>): <imperative description>` em inglês
  (`docs/COMMIT_CONVENTION.md`); escopos usuais `evolution`, `webview`, `app`,
  `settings`, `accounts`, `packaging`. Entregue sempre uma mensagem sugerida.

### Novo comando IPC (todos os passos)

1. `#[tauri::command]` em `commands/<domínio>.rs`, retornando
   `CommandResult<T>`; `async` se criar webview ou abrir diálogo (deadlock no
   WebView2).
2. Registrar em `generate_handler!` (`lib.rs`).
3. Nome em `UI_COMMANDS` ou `BRIDGE_COMMANDS` (`build.rs`) → gera
   `permissions/autogenerated/<nome>.toml`.
4. Conceder `allow-<nome-kebab>` em `capabilities/default.json` (`ui`,
   `panel`) ou `whatsapp-remote.json`.
5. Tipos com `#[derive(TS)] #[ts(export)]`; regenerar com
   `(cd src-tauri && cargo test)`.
6. Wrapper em `src/lib/ipc/index.ts`, ex.:
   `chats: (accountId: number) => invoke<ChatSummary[]>("evo_chats", { accountId })`.
   Argumentos de topo em camelCase no JS; campos de structs em snake_case.

### Rust

- Nomes de comando snake_case com prefixo de domínio: `evo_`, `wa_`,
  `spell_`, `report_`, `custom_`, `accounts_`, `layout_`, `settings_`, `app_`.
- Erros: `thiserror` por domínio (`EvoError`, `StoreError`, `AccountError`…),
  `anyhow` só em orquestração. `CommandError { message }` tem
  `From<E: Display>`, então `?` funciona nos comandos. Mensagens do servidor
  são sanitizadas; `Secret` imprime `<redacted>` em `Debug`.
- Async: `tauri::async_runtime::spawn`; `std::sync::{Mutex,RwLock}` para seções
  curtas, `tokio::sync::Mutex` quando o lock atravessa `.await`.
- Logs: macros `log` (`warn!`, `info!`), mensagens em minúsculas com `{e}`;
  sem `println!`. Release usa `panic = "abort"`.
- Enums persistidos em snake_case, **exceto `ProxyKind`** (`"NoProxy"`,
  `"Socks5Proxy"`…). Não normalize.
- Opções lidas na criação do webview (proxy, WebRTC estrito, engine) exigem
  `recreate_all`; na UI usam `RestartBar` quando só valem no próximo início.
- Evolution: `AdminKey` ≠ `InstanceToken` (tipos distintos, header `apikey`);
  base URL `https` (ou `http` em loopback); envio expirado vira `Ambiguous` e
  nunca é reenviado automaticamente; `GET /instance/qr` só com
  `LoggedIn == false`.
- `cfg` por plataforma: D-Bus, GTK stage, portal, WebKit permissions,
  notificações freedesktop são Linux-only; autostart plugin fora do Linux.

### Svelte / TypeScript

- Só Svelte 5 runes: `$props()` tipado inline, `$state`, `$derived`,
  `$effect`, `{@render}`/snippets, callbacks `onclose`/`onconfirm`. Nada de
  `export let`, `on:`, `<slot>` ou `svelte/store`.
- Estado de módulo em `*.svelte.ts` (`stores/settings.svelte.ts`:
  `startSettings()`, `patchSettings(patch)`). Eventos substituem o estado
  inteiro.
- Snapshot inicial de props vira estado local com `untrack(() => prop)`
  (`AccountEditDialog`, `ReportDialog`, `LanguagePicker`). Em `$effect`,
  leituras incidentais (funções chamadas que leem outro `$state`) também vão
  em `untrack`, senão viram dependências.
- Eventos: `on<T>(events.x, handler)` retorna `Promise<UnlistenFn>`; limpeza
  `return () => unlisten.then((f) => f())`.
- Erros de `invoke` chegam como objeto `{ message }`, não `Error`: leia
  `.message`, não `String(e)`.
- Diálogos e telas que cobrem o conteúdo chamam `layout.setOverlay(true)`
  (webviews nativos ficam por cima do HTML); menus são nativos (`MenuBar`),
  não HTML.
- Estilo: `<style>` escopado por componente; tokens globais em `src/app.css`
  (`--accent`, `--bg`, `--fg`, `--danger`) e classes `.btn`, `.muted`,
  `.info-box`.
- Nomes: componentes `PascalCase.svelte`; módulos `.ts` em lowercase/camelCase;
  lógica pura em `.ts` ao lado do componente, com `*.test.ts` colocalizado.
- i18n: `t("English text", { name })`, `tn(singular, plural, n)`; Rust usa
  `i18n::tr("…")`. `msgid` é o texto em inglês e deve dizer "FastZap" (o
  script renomeia ZapZap); `msgctxt` é descartado. Tabelas estáticas chamam
  `t()` no render. Não há extrator: strings novas ficam em inglês até existir
  entrada nos `.po`.
- `patchSettings` aceita `unknown` e o backend ignora chaves desconhecidas:
  chave digitada errada é no-op silencioso. Confira o campo Rust.
- Atalhos vivem em vários lugares que precisam ficar em sincronia:
  `webview/scripts/bridge.js` (`shortcutFor`) e `src/lib/shell/shortcuts.ts`
  (mapas idênticos), aceleradores de `MenuBar.svelte`,
  `shortcuts/ShortcutsDialog.svelte` e `app/actions.rs::dispatch_for`.

## Important Files

- Entradas: `src-tauri/src/main.rs` → `lib.rs::run()`; `src/routes/+layout.ts`
  (`ssr = false`), `+layout.svelte`, `+page.svelte`, `panel/+page.svelte`.
- `src-tauri/build.rs`: listas `UI_COMMANDS`/`BRIDGE_COMMANDS` (manifesto ACL)
  e embute `src/lib/i18n/catalogs/*.json` (diretório ausente = zero idiomas,
  sem erro).
- `src-tauri/tauri.conf.json`: identifier `io.github.quantmind-br.fastzap`
  (hífen); o `APP_ID` de runtime (`lib.rs`, pastas, D-Bus, keyring, Flatpak) é
  `io.github.quantmind_br.fastzap` (underscore). IDs usam `quantmind`, URLs e
  repositório oficial usam `quantforge-br/fastzap`. **Não unifique** —
  mudaria pastas de dados dos usuários.
- `src-tauri/.cargo/config.toml`: `TS_RS_EXPORT_DIR = ../src/lib/ipc/generated`.
- `src-tauri/src/app/state.rs` (`AppState`), `commands/mod.rs`
  (`CommandError`), `commands/bridge.rs`, `webview/navigation.rs`
  (allowlist), `settings/store.rs`, `secrets.rs`, `build_info.rs`
  (`FASTZAP_BUILD_*`), `update.rs`.
- `src-tauri/resources/hunspell-catalog.json` (gerado, versionado, via
  `include_str!`).
- `packaging/common/version.sh`, `build-info.sh`, `release-upload.sh`;
  `packaging/flatpak/io.github.quantmind_br.fastzap.yml`;
  `packaging/share/metainfo/io.github.quantmind_br.fastzap.metainfo.xml`.

## Runtime/Tooling Preferences

- **pnpm** fixado em `packageManager` (`pnpm@12.9.1`); CI usa Node 24. Não use
  npm/yarn nem altere o lockfile sem necessidade.
- Rust edition 2024, `rust-version = 1.85`, toolchain stable no CI.
- Linux precisa de `libwebkit2gtk-4.1-dev` (WebKitGTK ≥ 2.40),
  `libayatana-appindicator3-dev`, `librsvg2-dev`, `libssl-dev`,
  `libdbus-1-dev`.
- O build Rust embute `../build`: rode `pnpm build` antes de `cargo` num
  clone novo (o CI faz isso).
- **Bindings ts-rs**: o config de `src-tauri/.cargo/` só vale com o cwd em
  `src-tauri/`. `cargo test --manifest-path src-tauri/Cargo.toml` na raiz
  exporta para `src-tauri/bindings/` (sobras, não usadas) e **não** atualiza
  `src/lib/ipc/generated/`. Mudou tipo exportado? Rode
  `(cd src-tauri && cargo test)` e versione `generated/`; descarte arquivos
  novos em `src-tauri/bindings/`.
- Arquivos gerados e versionados: `src/lib/ipc/generated/`,
  `src/lib/i18n/catalogs/` (não edite à mão; `pnpm i18n` apaga e recria),
  `src-tauri/permissions/autogenerated/`. Ignorados: `build/`, `.svelte-kit/`,
  `src-tauri/gen/schemas`, `packaging/common/build-info.env`.

## Testing & QA

- **Vitest** (`vite.config.js`: `src/**/*.test.ts`, ambiente Node, sem DOM e
  sem mocks de Tauri): testa funções puras de `src/lib/`. Estilo
  `describe("<módulo>")` + `it("<frase de comportamento>")`. Componentes Svelte
  não têm testes.
- **Rust**: `#[cfg(test)] mod tests` no fim do arquivo; nomes descrevem o
  comportamento (`base_urls_require_https_except_loopback`); async com
  `#[tokio::test]`. Dev-deps: `wiremock`, `tempfile`.
  - Evolution REST com `wiremock::MockServer` (confere o header `apikey` por
    escopo); WebSocket contra `tokio-tungstenite` em `127.0.0.1:0`.
  - Segredos com `secrets::MemoryStore`; SQLite com
    `AccountStore::in_memory()`/`ChatStore::in_memory()`; arquivos em
    `tempfile::tempdir()`. Não altere variáveis de ambiente em testes.
- **CI** (`quality.yml`, push em `main` e PRs): `rust` (Ubuntu/Windows/macOS:
  `pnpm build`, fmt, clippy `-D warnings`, test), `frontend` (`pnpm check`,
  `pnpm test`), `packaging` (`version.sh --check`, `desktop-file-validate`,
  `appstreamcli validate --no-net`, `shellcheck`, `actionlint`). Sem meta de
  cobertura; `git diff --check` só local.
- **Manual** (`docs/testing.md` §Verificações manuais): webviews filhos,
  foco, tray, notificações e compositor exigem `pnpm tauri dev` com duas
  contas WhatsApp Web, deep link `xdg-open "whatsapp://send?phone=..."`,
  downloads, tema e grade. Sem teste automatizado: watchdog de 65 s do
  WebSocket, scripts injetados em `webview/scripts/`.
- Antes de entregar: `pnpm check`, `pnpm test`, `cargo fmt … --check`,
  `cargo clippy … -D warnings`, `cargo test …`, `git diff --check`.
