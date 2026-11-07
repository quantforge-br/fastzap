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
- `DownloadHistory`.

`evolution::Evolution` é registrado à parte (`app.manage`) e guarda uma
`evolution::Session` por conta Evolution Go ativa.

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
- `on_navigation`/`on_new_window` aplicando a allowlist de hosts. Como no
  ZapZap, `on_navigation` só bloqueia hosts de fora (o WhatsApp carrega
  frames de fundo, como `flows.whatsapp.net`); links abertos pelo usuário
  chegam por `on_new_window` e vão para o navegador. Popups
  para hosts permitidos abrem numa janela `popup-<n>` relacionada, com a mesma
  allowlist e sem acesso a comandos;
- `on_download` encaminhando para `downloads` (no Linux só o destino; a
  conclusão vem de `downloads::linux`);
- `proxy_url` quando o proxy global está ativo.

O handler de drag-and-drop do Tauri fica desativado nas contas `wa-*`, para
que o WhatsApp Web receba os arquivos pelo mecanismo HTML nativo. A webview
`ui` mantém os eventos Tauri para anexos do chat Evolution Go.

O frontend informa a área livre (à direita da sidebar) via
`layout_set_bounds`; o manager posiciona a conta ativa nessa área e oculta as
demais. O modo grade é a visão geral de contas do ZapZap: todos os child
webviews ficam ocultos e a UI local (`AccountsOverview.svelte`) mostra um
cartão por conta, WhatsApp Web ou Evolution Go, inclusive desativada, em
`grid_cols` colunas; o cartão ativa a conta (desativada pergunta antes de
reativar). Quando uma tela local precisa cobrir o conteúdo (configurações,
diálogos), o frontend chama `layout_set_overlay(true)` e os child webviews são
ocultados. Com grade ou overlay, o foco do teclado vai para o webview `ui`.

No Linux o Tauri empilha child webviews num `GtkBox` vertical e ignora
`set_position`. `webview::stage` insere um `GtkOverlay` na janela: o webview
`ui` é o filho principal (ocupa a janela inteira) e cada conta é um filho de
overlay posicionado por margens e `size_request`.

Se o processo web de uma conta termina sem ter sido pedido (travamento ou
limite de memória), o sinal `web-process-terminated` recarrega a página
depois de 1 s, como o `_on_render_crash` do ZapZap; sem isso a conta ficava
em branco até a webview ser recriada. Só no Linux.

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

Com `appearance.title_bar` desligado não há barra de título: nem decoração
nativa nem `TitleBar.svelte` (`window::native_decorations`), útil em
gerenciadores de janela lado a lado.

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

No Linux o Tauri mantém o contexto WebKit de cada conta pela vida do app, e
o wry conecta seus sinais de download de novo a cada webview recriada: a
mesma conclusão chegava uma vez por registro, e cada registro guarda um
único indicador de falha que nunca volta a falso, então depois de um
download falho ou cancelado (inclusive bloqueado) todos os seguintes da
conta eram dados como falhos e descartados. Por isso, no Linux, o
`on_download` só decide o destino (`Requested`); a conclusão do wry é
ignorada e `downloads::linux` observa o contexto uma vez por conta, com
estado de falha por download, e reporta a conclusão pelo caminho de
staging. Só um download ainda em recebimento casa com uma conclusão.

Mídias do chat Evolution Go (`evo_save_media`, clique no documento ou
"Download" no menu da mensagem) entram pelo mesmo fluxo com
`downloads::save_native`: os bytes são preparados no staging e seguem
comportamento, pasta, histórico e abertura automática. Como o download parte
de um clique do usuário, a pergunta de vários downloads não se aplica.

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
  `github.com/quantforge-br/fastzap/issues/new?title=…` (só o título).
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

`update` consulta `releases/latest` de `quantforge-br/fastzap` uma vez por
processo (timeout de 5 s, falhas silenciosas) e só quando `build_info` indica
build oficial do GitHub Actions do repositório oficial empacotado como DEB,
macOS ou Windows exe; Flatpak, AppImage, RPM, Snap e builds locais nunca
consultam. Rascunhos e pré-lançamentos são ignorados, versões são comparadas
numericamente e o link de notas só é aceito sob
`github.com/quantforge-br/fastzap/releases/`. O resultado fica em
`UpdateState` (`update_info`) e no evento `update://available`; a UI mostra
o botão ⬆ na barra lateral, que abre um diálogo (um popover ficaria sob os
webviews das contas), e uma linha na página Sobre.

## Onboarding e diálogos auxiliares

Ao carregar as preferências, `+page.svelte` decide uma vez por execução:
`onboarding.completed` falso abre `src/lib/onboarding/Onboarding.svelte`
(seis passos do `InitialSetupDialog` do ZapZap sobre um rascunho local,
`draft.ts`); "Pular" só marca `completed` e "Concluir" grava tudo num
`settings_patch`, forçando "Sair" ao fechar quando a bandeja fica
desligada. O FastZap não exibe pedido de doação. As ações
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

Contas do tipo Evolution Go não criam webview remoto: a área de conteúdo
mostra `src/lib/evolution/ChatView.svelte` (pareamento, lista de chats e
conversa), que conversa com o backend pelos comandos `evo_*` e pelo evento
`evolution://event` (`EvolutionEvent`).
O layout segue o WhatsApp Web: `ChatView` define as cores `--wa-*` (claro e
escuro por `prefers-color-scheme`) herdadas por `Conversation.svelte`,
`Avatar.svelte` e `Icon.svelte` (caminhos Material Icons); agrupamento de
balões, rótulos de dia, links e cores de remetente ficam em `format.ts`. O
`ChatSummary` traz `preview_status` (ticks da última mensagem nossa) e `own`
(conversa consigo mesmo); recibos e o fim de cada envio reemitem `Chats` para
atualizar os ticks.

- `evolution::client`: adaptador REST. `AdminKey` e `InstanceToken` são tipos
  distintos; respostas não-2xx viram `EvoError` antes de abrir
  `{message, data}` e mensagens do servidor passam por
  `reporting::sanitize`. Envios que expiram depois de sair viram
  `Ambiguous` e nunca são repetidos automaticamente.
- `evolution::ws`: um socket por instância (`/ws?token=<admin>&instanceId=`),
  ping a cada 20 s (65 s sem nenhum frame, nem pong, derrubam o socket;
  escritas e fechamento têm prazo de 10 s), backoff de 1 s a 60 s, frames
  sem limite de tamanho,
  decodificação dupla `{queue, payload}` e remoção de `instanceToken`. A URL
  carrega a chave admin e nunca é registrada. Um pedido de renovação
  (`Notify`) fecha o socket, espera o servidor encerrar a conexão (ele
  desregistra a instância logo antes) e reconecta sem `Down` nem backoff.
- `evolution::model`: converte eventos whatsmeow (mensagens, histórico,
  recibos, presença, conexão) em `Change`/`Event`, desembrulhando
  `ephemeralMessage`/`viewOnce`/`editedMessage`/`associatedChildMessage` e
  removendo miniaturas. Mensagens comerciais (`templateMessage`,
  `interactiveMessage`, `buttonsMessage`, `listMessage`) viram texto (título,
  corpo e rodapé), mídia do cabeçalho e `buttons` (`MessageButton`: só links
  `http(s)` abrem; respostas rápidas, linhas de lista e opções de enquete são
  rótulos); enquetes (`pollCreationMessage*`) são `poll`; respostas a botões
  e listas, texto; `ptvMessage`, vídeo. A mensagem de abertura de um álbum
  (`albumMessage`) não é guardada: as fotos chegam como mensagens próprias, e
  uma resposta ao álbum o cita como foto ou vídeo. O download envia ao
  servidor só o proto da mídia (`{"imageMessage": …}`) e só quando há
  `directPath`; a mídia que o WhatsApp serve apenas por `staticURL` (anúncios
  da conta oficial) o servidor não baixa.
- `evolution::store`: SQLite por conta em `<data>/evolution/<id>/chats.db`
  (chats, mensagens com chave `(chat, id)`, reações, contatos, meta). Status
  de entrega só avança (uma falha de envio tardia não rebaixa uma mensagem já
  confirmada). Cada página do journal é aplicada em ordem numa única
  transação junto com o cursor. O banco guarda em `meta` a origem
  (`bind:server` = servidor + instância, `bind:owner` = número pareado);
  quando ela muda, chats, mensagens, cursor e o cache de mídia são
  descartados. Mídia baixada fica em `<data>/evolution/<id>/media/`.
  `chats.archived` (schema 2) espelha `GET /chat/settings`, lido a cada
  conexão do socket e a cada 5 min. Chats arquivados ficam na pasta
  "Arquivadas", mesmo sem mensagens, e não entram no contador de não lidas.
  `lid_map` (schema 3) guarda pares LID → número aprendidos de
  `phoneNumberToLidMappings` e `pnJID`/`lidJID` do HistorySync e do
  `phoneJid` de `/chat/settings`; chats, remetentes, recibos e "digitando"
  que citam um LID conhecido são gravados sob o número, e aprender um par
  funde no chat do número o que estava sob o LID. Um HistorySync sob demanda
  (`syncType` 6) não sobrescreve não lidas. Ao migrar do schema 2 com chats
  LID, o cursor do journal é apagado para reler o journal e fundir os pares.
  `messages.quote_kind` (schema 4) guarda o tipo da mensagem citada; a
  migração o preenche, com o texto, a partir do proto de cada resposta. Ao
  ler uma página, a citação usa a mensagem citada quando ela está guardada
  (texto atual, autor, documento pelo nome) e, senão, a cópia que a resposta
  trouxe. `messages.buttons` (schema 5, JSON) guarda os botões; a migração
  relê do proto as mensagens guardadas como não suportadas e as citações
  delas, apaga as aberturas de álbum e refaz a prévia dos chats afetados.
- `evolution` (sessão): `sync` inicia/para sessões conforme as contas
  habilitadas, serializado por um lock de ciclo de vida; parar a sessão
  cancela o trabalho em andamento. Cada sessão confere a instância pela listagem admin e só chama
  `POST /instance/connect` quando o cliente está parado, o WebSocket está
  desligado ou falta uma categoria; o payload preserva webhook, RabbitMQ, NATS
  e eventos existentes. Depois de cada conexão do socket, lê
  `GET /chat/events` a partir do cursor salvo (o journal privado cobre os
  eventos perdidos com o socket fora); só um leitor por vez, 404 desliga o
  journal, 503/rede/timeout tentam de novo com backoff de 30 s a 30 min, e
  um backlog maior que 200 páginas continua em seguida. Com o socket de pé,
  o journal também é lido a cada 30 s (pulado se outra leitura está em
  curso; falhas esperam o próximo ciclo): o servidor guarda um socket por
  instância e, quando a limpeza atrasada de um socket antigo remove o
  registro do novo, o socket continua aberto (pongs chegam) sem receber
  eventos. Se a leitura grava mensagens que o socket não entregou e nenhum
  evento chegou desde a leitura anterior, a sessão pede um socket novo.
  `LoggedIn` só vale enquanto o cliente está autenticado. Sem socket
  (`Connected` falso), uma instância com JID pareado aparece como offline, e
  não como pareamento: o servidor mantém, sem tentar de novo, o cliente que
  não alcançou o WhatsApp ao iniciar, e não gera QR para ele (`logged_out`
  continua até "Link again"). "Tentar de novo" reinicia esse cliente com
  `POST /instance/forcereconnect/{id}` (chave admin, corpo com o número
  pareado), porque `POST /instance/reconnect` recusa cliente desconectado; o
  500 que o servidor devolve quando o cliente novo ainda não fez login 2 s
  depois não é erro, e a atualização seguinte decide o estado. Não lidas
  entram no `UnreadRegistry`
  (chats com não lidas, como o título do WhatsApp Web) e mensagens novas
  geram notificações pelo pipeline comum com `tag` = JID do chat; o clique
  emite `open_chat`.
- Segredos: a chave admin e o token da instância ficam só no keyring
  (`evolution-admin-key:<id>`, `evolution-instance-token:<id>`). A chave admin
  entra apenas por `evo_instances`/`evo_setup`; nenhum comando a devolve.
- `GET /instance/qr` reinicia um cliente logado para gerar QR novo, então
  QR e código por telefone só são pedidos com `LoggedIn` falso.
- Anexos por arraste: `Conversation.svelte` escuta `onDragDropEvent` na
  webview `ui`, converte as coordenadas físicas e aceita a soltura apenas
  sobre a conversa visível, online e fora de edição/envio. Os caminhos ficam
  numa fila local removível até confirmar em Enviar; trocar de conversa
  descarta a fila. `evo_send_file` recebe `path` opcional (nulo abre o seletor
  existente), valida arquivo regular e limite de 100 MB com leitura limitada
  no Rust, e reutiliza cache, outbox e envio de mídia. Os arquivos são enviados
  em sequência, com a mesma legenda e resposta; falha interrompe o lote e não
  recoloca o arquivo tentado na fila, pois o servidor pode tê-lo recebido.
  Sair da conversa interrompe os próximos envios, não o upload já iniciado.

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

- Identificadores: o App ID `io.github.quantmind_br.fastzap` nomeia Flatpak,
  `.desktop`, AppStream, nome D-Bus (`app::dbus` e
  `<APP_ID>.SingleInstance`), serviço do keyring e as pastas de
  configuração, dados e cache (`AppPaths`). O `identifier` do Tauri é
  `io.github.quantmind-br.fastzap`, porque o bundler recusa `_`; ele só
  aparece no bundle ID do macOS, no AUMID do Windows e na pasta do
  `tauri-plugin-window-state`. `enableGtkAppId` fica desligado, então o
  app_id Wayland/WM_CLASS é `fastzap` (`StartupWMClass=fastzap`).
- `pnpm tauri build` gera deb (`fast-zap`), rpm e AppImage no Linux, NSIS e
  MSI no Windows (arm64 só NSIS) e dmg universal no macOS, com assinatura
  ad-hoc e sem notarização. O `.desktop` de deb/rpm vem de
  `packaging/linux/fastzap.desktop.hbs` (`FastZap.desktop`, ícone `fastzap`,
  esquemas `whatsapp:` e `fastzap:`).
- `make install` usa `packaging/arch/install.sh` e `packaging/arch/PKGBUILD`
  para compilar a árvore local, converter o DEB em `dsr-fastzap-git` e
  instalar via pacman. Só a transação de instalação usa sudo; o pacote
  normaliza o lançador para `fastzap.desktop` e permanece separado do Flatpak.
- `packaging/share/`: `.desktop`, metainfo AppStream e ícone SVG nomeados
  pelo App ID, usados por Flatpak e snap; os ícones de `src-tauri/icons/`
  são gerados a partir desse SVG com `pnpm tauri icon`.
- `packaging/flatpak/`: manifesto sobre `org.gnome.Platform` 50 (WebKitGTK
  4.1), extensões `rust-stable` e `node24`, e `libayatana-appindicator` do
  submódulo `shared-modules` (o tray carrega a biblioteca em runtime). O
  módulo do app baixa crates e pacotes npm (`--share=network`), então serve
  para builds locais e de CI, não para o Flathub.
  No Flatpak com driver NVIDIA, `platform::disable_dmabuf_renderer` define
  `WEBKIT_DISABLE_DMABUF_RENDERER=1` antes do GTK; sem isso o Wayland derruba
  a conexão (Error 71) e o app fecha.
- `packaging/snap/`: `snapcraft.yaml` (core24, extensão `gnome`) copiado para
  `snap/` por `prepare.sh`; o slot D-Bus cobre o nome do single-instance.
- `packaging/common/`: `build-info.sh` marca builds oficiais
  (`FASTZAP_BUILD_*`, lidas em tempo de compilação por `build_info`) e
  `version.sh` imprime e confere a versão nos três manifestos.
- Workflows: `build-{deb,rpm,appimage,flatpak,snap,windows,macos}.yml`
  (manuais ou chamados), `release-candidate.yml` (tag `X.Y.Z-rc.N` e
  pre-release) e `release-deploy.yml` (release publicada). O setup comum está
  em `.github/actions/setup-build/`. O job `build` roda com `contents: read`
  e sem credencial persistida; só o job `publish` tem escrita e anexa os
  artefatos com `release-upload.sh`, que exige release existente e tag igual
  à versão (`v` e `-rc.N` opcionais). Actions de terceiros ficam fixadas por
  SHA.
