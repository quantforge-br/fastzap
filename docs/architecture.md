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
  Windows) ou `data_store_identifier` derivado do ID no macOS. O bundle
  exige macOS 14 (`bundle.macOS.minimumSystemVersion`), porque versões
  anteriores ignoram o identificador e o proxy (`macos-proxy`) e
  compartilhariam a sessão entre contas;
- `user_agent` de Chrome (o WhatsApp Web recusa o WebKit puro) ou o
  User-Agent escolhido por conta;
- `initialization_script` com a ponte (`webview/scripts/bridge.js`) e os
  scripts opcionais portados do ZapZap;
- `on_navigation`/`on_new_window` aplicando a allowlist de hosts; popups
  para hosts permitidos abrem numa janela `popup-<n>` relacionada, com a mesma
  allowlist e sem acesso a comandos;
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
conta chamadora é sempre derivada do rótulo do webview, nunca de argumentos.
`wa_shortcut` aceita só os atalhos listados em `REMOTE_ACTIONS` e os aplica à
conta chamadora; colar como texto simples lê o clipboard, então no Linux é
tratado no `key-press-event` nativo do WebKitGTK e nas demais plataformas
exige a janela em foco. `wa_profile_photo` só baixa de `*.whatsapp.net`, sem
seguir redirects e com teto de bytes. O seletor integrado abre um menu nativo
com as outras contas habilitadas. Configurações capturadas na criação do
webview (scripts de inicialização e proxy) recriam as contas quando mudam. O script de ponte
substitui `window.Notification` e observa `document.title` para extrair a
contagem de não lidas, como o ZapZap fazia pelo título da página.

### Painel de acesso rápido

`app::panel` cria na inicialização a janela oculta `panel` (rota `/panel`,
largura fixa de 320 px, fora da barra de tarefas) e decide a visibilidade
como o ZapZap: `always`, `when_hidden` (janela principal oculta ou
minimizada) ou `on_demand` (só por `show_panel`, na bandeja). Fechar dispensa
o painel até o modo ou o estado da janela principal mudar. `window::show` e
`window::hide` informam a visibilidade da janela principal, porque o tao
enfileira as requisições e `is_visible` só reflete a mudança depois. Fora do
Wayland, a posição padrão fica no canto superior direito da área de trabalho
(−15/+16), é lembrada após 500 ms sem movimento e, com `lock_position`, é
restaurada a cada `Moved`. A página usa os mesmos eventos de contas, não
lidas, layout e preferências da janela principal, e a capability `default`
cobre as webviews `ui` e `panel`.

### Decorações do cliente (CSR)

Com `appearance.csr`, a janela perde as decorações nativas
(`set_decorations`) e a UI desenha `TitleBar.svelte`: arrasto e duplo clique
via `data-tauri-drag-region`, botões com os temas do ZapZap (o tema vazio ou
desconhecido segue `XDG_CURRENT_DESKTOP`, lido por `app_desktop`) e, fora da
maximização, uma moldura de 4 px com bordas `startResizeDragging`. Como a UI
informa os limites da área de conteúdo, as webviews das contas descem abaixo
da barra sem cálculo no Rust.

### Tema

`app::theme` resolve o esquema efetivo. Em `auto`, a chave
`org.freedesktop.appearance color-scheme` do portal Settings prevalece quando
indica preferência, com `SettingChanged` monitorado (`monitor_system`); sem
preferência, vale o tema da janela informado pelo toolkit
(`WindowEvent::ThemeChanged`). O tema é aplicado à janela (decorações e
`prefers-color-scheme` da UI) e às páginas pelo `theme_controller.js`.

## Configurações e barra de menus

`src/lib/stores/settings.svelte.ts` mantém uma cópia reativa das
preferências (`settings_get` + evento `settings-changed`) e grava com
`settings_patch`. `src/lib/settings/` reúne a navegação (`pages.ts`, na
ordem do ZapZap), `SettingsView.svelte` e uma página por domínio em
`pages/`; os controles comuns ficam em `src/lib/components/settings/`.
Preferências lidas só na inicialização mostram `RestartBar`, que chama a
ação `restart`. `commands::app` expõe informações do build (`build_info`,
preenchido por `FASTZAP_BUILD_*`), senha do proxy no keyring, restauração
dos padrões, diálogos de arquivo/pasta e abertura de caminhos.

A página Contas e o menu de contexto da sidebar usam `src/lib/accounts/`.
Ícones seguem o formato do ZapZap (`accounts::icon` / `icon.ts`): SVG
gerado com cores aleatórias, foto PNG de 256 px recortada em círculo ou o
perfil `zapzap-account-image:v1:` que guarda as duas escolhas. A foto
escolhida passa por `account_photo_pick`, que abre o diálogo e normaliza a
imagem no Rust, sem expor o caminho à UI. A conta 1 (`storage-whats` no
ZapZap) não pode ser removida.

`MenuBar.svelte` reproduz os menus Arquivo/Exibir/Chat/Usuários/Ajuda com
menus nativos (`@tauri-apps/api/menu`), porque menus HTML ficariam sob as
webviews nativas das contas. A escala da interface vira zoom da webview
`ui` e multiplica o zoom de cada conta; a UI converte os limites que
informa para pixels lógicos da janela. `webview::manager::EngineOptions`
aplica no WebKitGTK a política de aceleração de hardware, a rolagem suave e
o WebRTC do proxy estrito; `disable_animations.js` entra nos scripts de
início quando `performance.disable_animations` está ativo.

## Downloads

`downloads` recebe o `on_download` de cada conta. O motor grava o arquivo
em `cache/download-staging/<seq>/`; só ao terminar valem as regras do
ZapZap: a pergunta de vários downloads (o primeiro da sessão sempre passa),
depois o comportamento (`dialog` via evento `downloads://prompt` e comando
`download_answer`, `automatic` ou `ask_every_time` com o diálogo nativo).
O arquivo vai para a pasta configurada com nome saneado
(`downloads::naming`) e sufixo ` (N)` quando o nome existe, entra no
histórico (`downloads.recent`, 100 entradas) e abre sozinho se
`downloads::sniff` confirmar PDF ou imagem pelo conteúdo e pela extensão.
Os comandos de abrir, revelar e excluir só aceitam caminhos do histórico.

## Permissões

`permissions` conecta o sinal `permission-request` do WebKitGTK em cada
webview de conta. Notificações são sempre concedidas; câmera, microfone,
tela, localização e bloqueio do ponteiro seguem `permissions.auto_grant`;
o resto pergunta à UI pelo evento `permissions://request`. O pedido
(GObject) fica numa tabela `thread_local` da thread GTK até
`permission_answer`, que responde via `run_on_main_thread`. Um "Sim" fica
em `AppState::permissions` (memória, por conta) e é esquecido quando a
webview é recriada. Outros tipos de pedido recebem a negação padrão.

## Customizações

`customizations` mantém a árvore do ZapZap em `data/customizations/`
(`global/` e `accounts/<chave>/`, com a conta 1 em `storage-whats`), cada
uma com `css/`, `js/` e `previews/css/`. Ao terminar cada carga da página,
`apply_to_webview` troca as tags `<style data-zapzap-custom-css>` e roda
cada JS com `sourceURL`. Os arquivos globais valem quando a categoria está
ativa; uma conta que não herda acrescenta os seus depois deles.
`normalize_userstyle` mantém só blocos `@-moz-document` de whatsapp.com
(estrito ao criar, editar e importar). `customizations::files` faz as
operações com lista branca de nomes e verificação de caminho; a importação
por URL aceita só HTTPS, 15 s e 2 MB, com prévia do userstyles.world. A
página `Customizations.svelte` grava arquivos na hora e as chaves de
ativação e herança só em Salvar.

## Corretor ortográfico

`spellcheck` usa Enchant + Hunspell do WebKitGTK. `lib.rs` define
`ENCHANT_CONFIG_DIR` como `<data>/dictionaries` antes do GTK iniciar, e os
dicionários do app ficam em `<data>/dictionaries/hunspell/<tag>.{aff,dic}`;
`/usr/share/hunspell` e `/usr/share/myspell` são listados como somente
leitura. As tags seguem a normalização do Enchant (`pt-PT` → `pt_PT`).

- `catalog`: catálogo embutido (`src-tauri/resources/hunspell-catalog.json`,
  gerado por `scripts/hunspell-catalog.mjs` a partir de um commit fixo do
  `wooorm/dictionaries`). Cada arquivo é conferido pelo tamanho e pelo SHA-1
  de blob do git antes de ser gravado via `.part` + rename.
- Seleção: até 10 idiomas, mapeados para tags instaladas (exata, depois
  idioma); vazia, cai no dicionário do locale do sistema e depois no primeiro
  instalado. Recentes: até 4, novos primeiro.
- `provision` instala uma vez o dicionário do locale do sistema
  (`spellcheck.provisioned_locale`); falhas são tentadas de novo no próximo
  início.
- `apply_all` aplica idiomas e estado ao `WebContext` de cada conta; roda na
  criação do webview, em mudanças de `spellcheck` e após instalar/remover.
- O menu de contexto (`linux.rs`) remove itens de navegação e inspeção e
  acrescenta "Verificar ortografia" (ação `GAction` com estado) e
  "Idiomas…", que emite a ação de UI `spellcheck_languages`.
- UI: seção da página Idioma e Download, `LanguagePicker` e
  `DictionaryManager`; o evento `spellcheck://changed` atualiza ambos.

## Relatórios de problemas

`reporting` segue o `core/reporting` do ZapZap; nada sai do dispositivo sem
revisão do usuário.

- `sanitize`: remove caminhos pessoais, query/fragmento de URLs, e-mails, IDs
  de chat, tokens, cookies, UUIDs e telefones; em documentos JSON descarta
  chaves com cara de segredo e preserva IDs, versões e horários bem formados.
- `document`: relatório manual (`manual_problem`), pânico
  (`automatic_crash`) e sessão anterior não encerrada
  (`unexpected_shutdown`); Markdown em inglês e URL
  `github.com/quantmind-br/fastzap/issues/new?title=…` (só o título).
- `store`: `<data>/reports/<id>.json`, no máximo 20 e 30 dias; status
  `pending`, `pending_review`, `copied`, `opened_on_github`.
- `session`: `.session-active` com fases `startup`/`running`/`shutdown`
  (`crashed` após um pânico); removido no `RunEvent::Exit`. Um marcador
  sobrando vira relatório na próxima execução quando
  `reporting.crash_prompts` está ligado. O `TeeLogger` mantém avisos e erros
  da sessão (16 kB) para a opção de logs; o panic hook guarda o último pânico.
- UI: página Relatar um problema (`debugging`), `ReportDialog` (formulário e
  revisão; a UI copia o Markdown e o backend abre o GitHub),
  `ReportsDialog` e o aviso de inicialização (só após o onboarding).

## Verificação de atualizações

`update` consulta `releases/latest` de `quantmind-br/fastzap` uma vez por
processo (timeout de 5 s, falhas silenciosas) e só quando `build_info` indica
build oficial do GitHub Actions do repositório oficial empacotado como DEB,
macOS ou Windows exe; Flatpak, AppImage, RPM, Snap e builds locais nunca
consultam. Rascunhos e pré-lançamentos são ignorados, versões são comparadas
numericamente e o link de notas só é aceito sob
`github.com/quantmind-br/fastzap/releases/`. O resultado fica em
`UpdateState` (`update_info`) e no evento `update://available`; a UI mostra
o botão ⬆ na barra lateral, que abre um diálogo (um popover ficaria sob os
webviews das contas), e uma linha na página Sobre.

## Onboarding, doação e diálogos auxiliares

Ao carregar as preferências, `+page.svelte` decide uma vez por execução:
`onboarding.completed` falso abre `src/lib/onboarding/Onboarding.svelte`
(seis passos do `InitialSetupDialog` do ZapZap sobre um rascunho local,
`draft.ts`); "Pular" só marca `completed` e "Concluir" grava tudo num
`settings_patch`, forçando "Sair" ao fechar quando a bandeja fica
desligada. Com o onboarding concluído e `donation_message_hidden` falso,
aparece o lembrete `DonationReminder`. A página `DonationView` (painel
`donate`) lista os métodos do ZapZap; a UI só envia o id e
`donation_open` resolve a URL fixa em `donation.rs`. As ações
`open_chat_by_number` (`src/lib/chat/`, validação espelhando
`webview::deeplink`) e `show_shortcuts` (`src/lib/shortcuts/`) abrem
diálogos. Todos contam como sobreposição, porque as webviews nativas
ficariam por cima do HTML.

## Traduções

`po/*.po` vem do ZapZap (mesmo domínio e idiomas). `pnpm i18n`
(`scripts/po2json.mjs`) gera `src/lib/i18n/catalogs/<locale>.json`, versionados:
`{plural, messages, plurals}`, sem entradas `fuzzy`, com "ZapZap" trocado por
"FastZap". O Rust embute os catálogos no build (`build.rs`) e usa `i18n::tr`
na bandeja, menus e notificações; a UI usa `t`/`tn` de `$lib/i18n`, com
`Plural-Forms` avaliado sem `eval` (`plural.ts`). O comando `app_locale`
resolve `system` para o locale do SO; o layout troca o catálogo ao vivo
quando `interface_language` muda.

## Bandeja e badge

`tray` renderiza com `resvg` os SVGs do ZapZap (`default`, `symbolic_light`,
`symbolic_dark`) a 128 px, com o contador vermelho (máximo `999`). O total soma
só contas habilitadas e vira zero com `notification_counter` desligado; o mesmo
valor alimenta `Window::set_badge_count` (Linux/macOS); no Windows, onde o
badge não existe, vira um ícone de sobreposição na barra de tarefas (disco
vermelho de 32 px, `99+` acima de 99). O menu é reconstruído quando idioma,
mudo ou painel flutuante mudam. Em bandejas StatusNotifierItem (Linux) cliques
no ícone não chegam ao app; só o menu.

## Notificações

`bridge.js` substitui `window.Notification` e envia `wa_notify`/
`wa_notification_close`. `notifications::policy` aplica `notifications.*`, o
DND da conta e o modo silencioso (`sound` desligado ou `audio_muted`). O
backend é escolhido como no ZapZap: portal no Flatpak,
`org.freedesktop.Notifications` (via `zbus`, com `ActionInvoked` e
`ActivationToken`) nas demais instalações Linux e `tauri-plugin-notification`
em Windows/macOS, que não informa cliques. Uma notificação nova com a mesma
`tag` fecha as anteriores. O clique mostra a janela com o token de ativação,
ativa a conta e chama `notificationClicked` na página, que abre a conversa.
Fotos circulares ficam em `<cache>/notification-images/`.

## Contas Evolution Go

Contas do tipo Evolution Go não criam webview remoto. `evolution::session`
mantém, por conta, o cliente REST, a conexão WebSocket e o store SQLite de
mensagens; a UI Svelte em `src/routes/chat/` consome os comandos
`evo_*` e o evento `evolution://event`. Detalhes em
`docs/evolution-go.md` (a ser escrito com a feature).

## Inicialização e encerramento

1. `lib::run` interpreta a CLI (`app::cli`, no estilo `parse_known_args`:
   `--setSettings CHAVE VALOR` com as chaves `grupo/chave` do ZapZap,
   `--wayland`, `--hideStart`; os demais argumentos são links). Antes do GTK,
   define `GDK_BACKEND` a partir de `--wayland`, de um `GDK_BACKEND` já
   exportado ou de `system.display_backend` (lido sem reparar o arquivo).
2. `tauri-plugin-single-instance` entrega os argumentos de um segundo
   lançamento a `app::lifecycle::on_second_instance`: aplica `--setSettings`,
   reinicia com `fastzap://restart`, abre o primeiro link `whatsapp:`/wa.me
   válido e mostra a janela.
3. `setup` carrega preferências, executa a migração do ZapZap uma única vez,
   aplica `--setSettings`, cria janela, child webviews e tray e instala a
   ponte de `SIGTERM`/`SIGINT` (`signal-hook` → `AppHandle::exit`). No
   Flatpak, `app::dbus` assume o nome `io.github.quantmind_br.fastzap` e
   exporta `org.freedesktop.Application` (`Activate`, `Open`,
   `ActivateAction`) em `/io/github/quantmind_br/fastzap`, repassando o token
   de ativação (`activation-token` ou `desktop-startup-id`) para a janela.
4. A janela começa oculta com `start_background` ou `--hideStart`, exceto
   enquanto a configuração inicial não foi concluída. Links do primeiro
   lançamento esperam o carregamento da página (`eval_when_ready`).
5. `tauri-plugin-window-state` restaura tamanho, posição e maximização da
   janela `main` (nunca a visibilidade) e salva ao sair.
6. Fechar a janela pede `app::lifecycle::request_close`: com
   `confirm_on_close` a UI pergunta (com "Don't ask again", também para Sair
   da bandeja e `Ctrl+Q`); depois a janela vai para a bandeja, ou o app sai se
   `quit_on_close` estiver ativo ou se o pedido foi Sair.
7. `system.start_system` é aplicado ao iniciar e a cada mudança
   (`autostart`): no Flatpak, portal `Background` (`RequestBackground` com
   `fastzap --hideStart`); em outros Linux, a entrada
   `$XDG_CONFIG_HOME/autostart/io.github.quantmind_br.fastzap.desktop`
   apontando para o AppImage ou o executável atual; no Windows e no macOS,
   `tauri-plugin-autostart` (chave Run e LaunchAgent).
8. Em `RunEvent::Exit` as notificações exibidas são removidas e o nome D-Bus
   é liberado.

## Empacotamento

`tauri build` gera deb, rpm e AppImage no Linux, NSIS/MSI no Windows e dmg no
macOS. O manifesto Flatpak (`packaging/flatpak/`) usa `org.gnome.Platform`
por causa do WebKitGTK. Arquivos de desktop, AppStream e ícones ficam em
`packaging/share/`.
