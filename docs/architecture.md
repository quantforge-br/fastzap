# Arquitetura do FastZap

## Visão geral

FastZap é um aplicativo Tauri v2. O processo Rust (`src-tauri/`) é a autoridade
sobre estado, preferências, contas, segredos e integrações nativas. A interface
local é uma SPA SvelteKit (`src/`, `adapter-static`) carregada no webview
`ui` da janela `main`. Cada conta do tipo WhatsApp Web é um **child webview** separado,
carregando `https://web.whatsapp.com/`, posicionado dentro da mesma janela.

```text
src-tauri/src/main.rs
        |
        v
lib.rs::run()               plugins, setup, eventos de ciclo de vida
        |
        +--> app/            CLI, single instance, D-Bus, sinais, janela, atalhos
        +--> settings/       domínios tipados + store JSON versionado
        +--> migration/      importação única do ZapZap
        +--> accounts/       contas (SQLite) e avatares
        +--> webview/        child webviews, scripts injetados, ponte remota
        +--> evolution/      cliente REST, WebSocket, store de mensagens
        +--> notifications/  política + backends por plataforma
        +--> tray/ downloads/ permissions/ proxy/ customizations/
             spellcheck/ reporting/ update/ autostart/ i18n/
        |
        v
commands/                   superfície IPC invocada pelo frontend
```

Regras de dependência:

- `settings`, `secrets`, `paths` e `i18n` não dependem de nenhum outro módulo
  de feature.
- Features dependem de `settings`/`accounts`, nunca umas das outras
  diretamente; a coordenação acontece em `app::state::AppState` ou via eventos.
- `commands/` é a única camada que conhece o formato das chamadas IPC.

## Estado compartilhado

`app::state::AppState` é registrado com `app.manage()` e reúne:

- `SettingsStore` (`RwLock<Settings>` + caminho do JSON);
- `AccountStore` (SQLite `accounts.db`);
- `UnreadRegistry` (contagem por conta → tray, badge e sidebar);
- `EvolutionHub` (um `EvolutionSession` por conta Evolution Go);
- `DownloadHistory`.

Mudanças de estado relevantes para a UI são emitidas como eventos Tauri com
nomes estáveis (`settings://changed`, `accounts://changed`,
`unread://changed`, `evolution://event`, `downloads://changed`).

## Janela principal e contas WhatsApp Web

A janela `main` é criada em `app::window` com um webview local de tela cheia
(sidebar, overlays, configurações, chat nativo). Para cada conta WhatsApp Web
habilitada, `webview::manager` cria um child webview com `Window::add_child`
(feature `unstable` do Tauri):

- `data_directory` exclusivo em `<app_data>/profiles/<account-id>` (Linux e
  Windows) ou `data_store_identifier` derivado do ID (macOS 14+);
- `user_agent` de Chrome (o WhatsApp Web recusa o WebKit puro) ou o
  User-Agent escolhido por conta;
- `initialization_script` com a ponte (`webview/scripts/bridge.js`) e os
  scripts opcionais portados do ZapZap;
- `on_navigation`/`on_new_window` aplicando a allowlist de hosts;
- `on_download` encaminhando para `downloads`;
- `proxy_url` quando o proxy global está ativo.

O frontend informa a área livre (à direita da sidebar) via
`layout_set_bounds`; o manager posiciona a conta ativa nessa área e oculta as
demais. No modo grade, as contas visíveis são distribuídas em `grid_cols`
colunas. Quando uma tela local precisa cobrir o conteúdo (configurações,
diálogos), o frontend chama `layout_set_overlay(true)` e os child webviews são
ocultados.

No Linux o Tauri empilha child webviews num `GtkBox` vertical e ignora
`set_position`. `webview::stage` insere um `GtkOverlay` na janela: o webview
`ui` é o filho principal (ocupa a janela inteira) e cada conta é um filho de
overlay posicionado por margens e `size_request`.

### Ponte remota

`build.rs` declara todos os comandos do app em um `AppManifest`, então o ACL
do Tauri exige permissão explícita para cada um. `capabilities/default.json`
libera os comandos locais apenas para o webview `ui`;
`capabilities/whatsapp-remote.json` permite que webviews `wa-*` em
`https://web.whatsapp.com/*` invoquem apenas os comandos `wa_*` de
`commands::bridge` (título, atalhos, eventos do tema, seletor de contas e foto
de perfil; notificações entram com a fase de integração com o desktop). A
conta chamadora é sempre derivada do rótulo do webview, nunca de argumentos. O script de ponte
substitui `window.Notification` e observa `document.title` para extrair a
contagem de não lidas, como o ZapZap fazia pelo título da página.

## Contas Evolution Go

Contas do tipo Evolution Go não criam webview remoto. `evolution::session`
mantém, por conta, o cliente REST, a conexão WebSocket e o store SQLite de
mensagens; a UI Svelte em `src/routes/chat/` consome os comandos
`evo_*` e o evento `evolution://event`. Detalhes em
`docs/evolution-go.md` (a ser escrito com a feature).

## Inicialização e encerramento

1. `main.rs` interpreta a CLI (`app::cli`) e aplica variáveis que precisam
   existir antes do GTK (`GDK_BACKEND`, flags do WebKit).
2. `tauri-plugin-single-instance` encaminha argumentos e deep links para a
   instância em execução.
3. `setup` carrega preferências, executa a migração do ZapZap uma única vez,
   abre o banco de contas, cria tray, janela e child webviews, inicia sessões
   Evolution Go habilitadas, D-Bus (Flatpak) e a ponte de `SIGTERM`.
4. A visibilidade inicial respeita `--hideStart` e `start_background`.
5. Em `RunEvent::ExitRequested` as notificações são removidas, as sessões
   Evolution Go fechadas e o marcador `.session-active` apagado.

## Empacotamento

`tauri build` gera deb, rpm e AppImage no Linux, NSIS/MSI no Windows e dmg no
macOS. O manifesto Flatpak (`packaging/flatpak/`) usa `org.gnome.Platform`
por causa do WebKitGTK. Arquivos de desktop, AppStream e ícones ficam em
`packaging/share/`.
