# IDEATION_UI_UX — Auditoria de UI/UX do FastZap

- Data: 2026-11-05 · Commit auditado: `0d72a38` (branch `main`, árvore limpa)
- Escopo: shell principal, onboarding, menu bar nativo, Configurações (Contas, Aparência),
  conta Evolution Go (configuração, lista de conversas, conversa, composer, estados offline),
  visão geral de contas (grade), Downloads, diálogos (atalhos, enviar para número,
  confirmação de fechamento), painel de acesso rápido.
- Evidências: `.ideation/uiux-screenshots/` (70 capturas), `.ideation/uiux-snapshots/`
  (10 árvores de acessibilidade), `.ideation/uiux-captures/` (6 capturas textuais).
  QR codes do WhatsApp Web foram cobertos com uma caixa cinza "QR REDACTED"; todos os
  contatos, números e mensagens são fictícios.

## Executive Summary

O FastZap tem uma base visual coerente (tokens em `src/app.css`, paleta do WhatsApp no
chat Evolution, tema escuro funcional) e os fluxos principais funcionam com mouse na
janela padrão de 1000x700. Os problemas mais graves aparecem fora desse caminho feliz:

1. **Ações destrutivas sem confirmação nem desfazer** — "Delete for everyone" apaga a
   mensagem para todos com um clique, e o ícone de lixeira em Downloads apaga o arquivo
   do disco permanentemente (`std::fs::remove_file`, sem lixeira). Ambos comprovados em
   runtime (UIUX-001).
2. **O composer some no tamanho mínimo de janela e em 200 % de escala** — a grade do chat
   Evolution mantém a lista com no mínimo 300 px e deixa ~116 px para a conversa; a caixa
   de texto e o botão de enviar ficam fora da tela (UIUX-002).
3. **Acessibilidade por teclado quebrada em pontos centrais** — diálogos modais não movem
   nem prendem o foco (o Tab sai do onboarding e abre o menu "File" atrás dele), os
   botões por mensagem recebem foco com `opacity: 0`, e as ações da lista de Downloads são
   inalcançáveis pelo teclado (UIUX-003, UIUX-004).
4. **Contraste insuficiente no tema claro** — links (2,13:1), nomes de remetentes e autores
   de citação (1,15–4,00:1), horários/contadores de não lidas (3,01:1), botões primários
   (3,03:1) e item ativo das Configurações (2,50:1) (UIUX-005).
5. **Interface em pt-BR parcialmente em inglês** — 157 de 735 textos traduzíveis (21 %)
   não têm tradução pt_BR, incluindo todo o chat Evolution; o painel de acesso rápido e a
   confirmação de fechamento nem passam por `t()`; erros do servidor aparecem crus, em
   inglês e minúsculas (UIUX-007, UIUX-008).

Recomendação estrutural principal: criar dois primitivos compartilhados — um diálogo
modal (foco inicial, armadilha de foco, `inert` no fundo, restauração) e um menu
(teclado completo e posicionamento dentro da viewport) — e definir um contrato
responsivo com breakpoints para chat, Configurações e grade.

## Visual Inspection Log

| Campo | Valor |
|---|---|
| UI Type | App desktop Tauri 2: SPA SvelteKit/Svelte 5 no webview `ui` (WebKitGTK), menus nativos GTK (muda), webviews nativos do WhatsApp Web por conta, janela `panel` separada |
| Environment | Arch Linux x64 (kernel 7.2.14); Xvfb `:93` 1440x900x24 **sem gerenciador de janelas**; `dbus-run-session` privado; HOME/XDG privados em `/tmp/fz-uiux/profile`; gnome-keyring privado; ALSA `null`; `LANG=en_US.UTF-8`. Harness Chromium (agent-browser 0.34.0, sessão `uiux-audit-fastzap`) para a mesma build da SPA |
| Tooling | `xdotool` (teclado, mouse, redimensionamento), ImageMagick `import` (captura por janela), agent-browser (`snapshot`, `a11y` com axe-core 4.12.1, `eval` para estilos computados/`getBoundingClientRect`, `press`), mock aiohttp do Evolution Go em `127.0.0.1:18080` |
| Launch Command | `pnpm build && (cd src-tauri && cargo build --release --features tauri/custom-protocol)`; app: `dbus-run-session -- bash /tmp/fz-uiux/run-app.sh` (executa `src-tauri/target/release/fastzap` com ambiente isolado); harness: `python3 serve.py 4173` (SPA `build/` com fallback) + `agent-browser --session uiux-audit-fastzap open --init-script tauri-mock.built.js 'http://127.0.0.1:4173/?s=evo'` (mock de `__TAURI_INTERNALS__` baseado em `@tauri-apps/api/mocks`) |
| Role/Fixture | Primeiro uso real (onboarding completo); conta WhatsApp Web padrão (não logada); conta Evolution Go "Audit Evolution" contra o mock (11 conversas fictícias, grupo, arquivada, documento, recibos lido/entregue); 3ª conta criada pelo painel. Harness: 3 contas (uma desativada com "não perturbe"), 11 conversas, 11 mensagens cobrindo `read`/`delivered`/`failed`/`unknown`, citação, mídia expirada, documento, link, reação |
| Screens | 14 superfícies (tabela abaixo); 70 capturas, 10 snapshots de acessibilidade, 6 capturas textuais |
| Sizes | 1000x700 (padrão, `src-tauri/src/app/window.rs:11`), 640x480 (compacto), 480x400 (mínimo permitido, `window.rs:24`), 320x420 (painel); escala de interface 200 % a 1000x700 (≈500x350 px CSS); tema claro e escuro; locale en_US e pt_BR |

### Cobertura

R = app real (release, Xvfb). C = harness Chromium (mesma build da SPA, IPC simulado).

| Superfície | Estados | Tamanhos | Fonte | Evidência |
|---|---|---|---|---|
| Onboarding (6 passos) | cada passo, foco por teclado, fuga do modal, menu atrás do modal, compacto | 1000x700, 640x480, 480x400 | R + C | `onboarding-basics-1000x700.png`, `onboarding-notifications-1000x700.png`, `onboarding-background-1000x700.png`, `onboarding-files-1000x700.png`, `onboarding-permissions-1000x700.png`, `onboarding-finish-1000x700.png`, `onboarding-tab1-1000x700.png`, `onboarding-tab3-1000x700.png`, `onboarding-tab-next-1000x700.png`, `onboarding-tab-escape-behind-1000x700.png`, `onboarding-tab-escape-behind2-1000x700.png`, `onboarding-behind-file-menu-open-1000x700.png`, `onboarding-background-640x480.png`, `onboarding-background-480x400.png`, `uiux-snapshots/onboarding-basics-1000x700.txt` |
| Shell + WhatsApp Web (login) | padrão, compacto, após Configurações, sem sidebar/menu bar | 1000x700, 480x400 | R | `shell-whatsapp-web-default-1000x700.png`, `shell-whatsapp-web-compact-480x400.png`, `shell-after-settings-1000x700.png`, `shell-no-sidebar-no-menubar-1000x700.png`, `shell-no-chrome-evolution-1000x700.png`, `shell-no-chrome-ctrlp-recovery-1000x700.png`, `uiux-snapshots/shell-evo-idle-1000x700.txt` |
| Menu bar nativo | menu View aberto, menu File aberto | 1000x700 | R | `menubar-view-open-1000x700.png`, `onboarding-behind-file-menu-open-1000x700.png` |
| Configurações › Contas | 1, 2 e 3 contas, compacto, conta nova | 1000x700, 640x480, 480x400 | R + C | `settings-accounts-1000x700.png`, `settings-accounts-two-1000x700.png`, `settings-accounts-640x480.png`, `settings-accounts-480x400.png`, `panel-new-account-result-1000x700.png`, `uiux-snapshots/settings-accounts-1000x700.txt` |
| Configurações › Aparência | padrão, sidebar off, sidebar+menu off, painel on, 200 % | 1000x700 | R | `settings-appearance-1000x700.png`, `settings-sidebar-off-1000x700.png`, `settings-sidebar-menubar-off-1000x700.png`, `settings-appearance-panel-on-1000x700.png`, `settings-appearance-scale200-1000x700.png` |
| Diálogo Evolution Go | vazio, erro de credencial, instâncias carregadas | 1000x700 | R | `evo-setup-empty-1000x700.png`, `evo-setup-error-1000x700.png`, `evo-setup-instances-1000x700.png` |
| Lista de conversas Evolution | ociosa, offline + Retry, 200 %, pt_BR | 1000x700 | R + C | `evo-chatlist-idle-1000x700.png`, `evo-offline-banner-1000x700.png`, `evo-offline-retry-pressed-1000x700.png`, `evo-chatlist-scale200-1000x700.png` |
| Conversa Evolution | padrão, hover, menu de ações (cortado/rolado), exclusão, composer digitado/enviado, foco por teclado, Escape, compacto, 200 %, escuro, pt_BR, estados de entrega/mídia/citação | 1000x700, 640x480, 480x400 | R + C | `evo-conversation-default-1000x700.png`, `evo-conversation-hover-own-1000x700.png`, `evo-conversation-own-actions-menu-1000x700.png`, `evo-conversation-own-actions-menu-scrolled-1000x700.png`, `evo-conversation-after-delete-click-1000x700.png`, `evo-composer-typed-1000x700.png`, `evo-composer-after-send-1000x700.png`, `evo-conversation-tab12-focus-hidden-1000x700.png`, `evo-conversation-tab13-focus-hidden-1000x700.png`, `evo-conversation-tab13-enter-menu-1000x700.png`, `evo-conversation-menu-after-escape-1000x700.png`, `evo-conversation-640x480.png`, `evo-conversation-480x400.png`, `evo-conversation-scale200-1000x700.png`, `evo-conversation-dark-1000x700.png`, `evo-conversation-ptbr-chromium-1000x700.png`, `evo-conversation-states-chromium-1000x700.png`, `uiux-snapshots/evo-conversation-1000x700.txt` |
| Visão geral de contas (grade) | padrão, compacto | 1000x700, 480x400 | R + C | `grid-overview-1000x700.png`, `grid-overview-480x400.png`, `uiux-snapshots/grid-overview-1000x700.txt` |
| Downloads | vazio, prompt de download, concluído, hover, após excluir, lista com estados | 1000x700 | R + C | `downloads-empty-1000x700.png`, `download-prompt-dark-1000x700.png`, `downloads-completed-dark-1000x700.png`, `downloads-row-hover-dark-1000x700.png`, `downloads-after-delete-click-dark-1000x700.png`, `downloads-list-chromium-1000x700.png`, `downloads-list-hover-chromium-1000x700.png`, `uiux-snapshots/downloads-list-1000x700.txt` |
| Atalhos de teclado | aberto | 1000x700 | R + C | `shortcuts-dialog-1000x700.png` |
| Enviar mensagem para número | aberto, Tab | 1000x700 | C | `open-chat-dialog-chromium-1000x700.png`, `uiux-snapshots/open-chat-dialog-1000x700.txt` |
| Confirmação de fechamento | aberto em pt_BR | 1000x700 | C | `close-confirm-ptbr-chromium-1000x700.png`, `uiux-snapshots/close-confirm-ptbr-1000x700.txt` |
| Painel de acesso rápido | padrão, estados, pt_BR, "New account" | 320x420 | R + C | `panel-default-320x420.png`, `panel-states-chromium-320x420.png`, `panel-ptbr-chromium-320x420.png`, `uiux-snapshots/panel-default-320x420.txt`, `uiux-snapshots/panel-ptbr-320x420.txt` |

Capturas de suporte: `uiux-captures/keyboard-sequences.txt` (sequências K1–K9),
`uiux-captures/contrast-and-target-measurements.txt`, `uiux-captures/axe-evo-conversation-1000x700.txt`,
`uiux-captures/evo-delete-no-confirm-mock-log.txt`, `uiux-captures/downloads-delete-file-no-confirm.txt`,
`uiux-captures/i18n-ptbr-missing-msgids.txt`.

## Visual Inspection Limitations

- **L1 — Sem gerenciador de janelas.** O Xvfb não tem WM nem compositor: janelas abrem em
  0,0, sem decorações nativas; tooltips aparecem como caixas pretas (artefato do ambiente,
  não do app). Decorações próprias (CSR) e barra de título não foram avaliadas.
- **L2 — Bandeja, notificações e atalhos globais não inspecionáveis.** A sessão D-Bus
  privada não tem host StatusNotifier nem daemon de notificações.
- **L3 — WhatsApp Web logado inacessível.** Logar exigiria uma conta real; só a página de
  login foi vista (QR censurado). Scripts injetados (seletor de contas integrado, lista
  redimensionável, atalhos da página) não foram exercitados.
- **L4 — Evolution Go apenas contra mock local**, conforme `AGENTS.md`. O estado de
  pareamento (QR do `PairingPanel`) não foi exibido porque a instância do mock já estava
  conectada.
- **L5 — Árvore AT-SPI não capturada** (pyatspi ausente). A árvore de acessibilidade do
  Chromium, sobre a mesma build da SPA, foi usada como aproximação; a exposição no
  WebKitGTK pode diferir em detalhes [INFERENCE]. Nenhum leitor de tela (Orca) foi executado.
- **L6 — Harness com IPC simulado.** Comandos e eventos do backend são fixtures; o
  comportamento visual é o da SPA real, mas fluxos que dependem do Rust só foram
  confirmados no app real quando indicado.
- **L7 — Contraste por estilo computado**, não por amostragem de pixels; antialiasing e
  imagens de fundo do chat não entram no cálculo.
- **L8 — 200 % via a opção "Interface scale"** do app, não via escala do sistema.
- **L9 — Superfícies não inspecionadas (11):** menu da bandeja; notificações; WhatsApp Web
  logado e scripts injetados; decorações CSR; arrastar e soltar anexos; diálogo de
  atualização; diálogo de recuperação após falha; "Report a problem"; editor de
  customizações; diálogos de permissão do WhatsApp Web; páginas de Configurações além de
  Contas e Aparência (Notificações, Permissões, Sistema, Idioma, Privacidade, Avançado,
  Desempenho, Sobre).

## Issues Found

### High

#### UIUX-001 — Exclusões irreversíveis com um clique, sem confirmação nem desfazer

- **Category:** Interaction
- **Priority Rationale:** perda de dados irreversível (mensagem apagada para todos os
  participantes; arquivo removido do disco sem lixeira) a partir de um único clique em
  alvos pequenos e próximos de ações inofensivas.
- **Confidence:** Alta — ambos os caminhos executados no app real.
- **Evidence:**
  - `.ideation/uiux-screenshots/evo-conversation-own-actions-menu-scrolled-1000x700.png`
    → `.ideation/uiux-screenshots/evo-conversation-after-delete-click-1000x700.png`;
    `.ideation/uiux-captures/evo-delete-no-confirm-mock-log.txt` (`POST /message/delete`
    enviado imediatamente pelo clique).
  - `.ideation/uiux-screenshots/downloads-row-hover-dark-1000x700.png` →
    `.ideation/uiux-screenshots/downloads-after-delete-click-dark-1000x700.png`;
    `.ideation/uiux-captures/downloads-delete-file-no-confirm.txt` (arquivo existia antes do
    clique e não existe depois; nada na lixeira).
  - Código: `src/lib/evolution/Conversation.svelte:567-571`,
    `src/lib/downloads/DownloadsView.svelte:91`, `src-tauri/src/commands/downloads.rs:63`.
- **Affected Journey/Screens:** conversa Evolution › menu de ações › "Delete for everyone";
  Downloads › ícone 🗑 (e item "Delete file" do menu de contexto, `DownloadsView.svelte:51`).
- **Affected Code:** `Conversation.svelte` (`act(() => evolution.remove(target, m.id))`),
  `DownloadsView.svelte` (`downloads.deleteFile(item.path)`), `commands/downloads.rs`
  (`std::fs::remove_file`).
- **Current State:** o item de menu "Delete for everyone" fica logo abaixo de "Edit"/"Send
  again" e dispara a revogação no clique. O 🗑 aparece só no hover, ao lado de 📂, e apaga
  o arquivo permanentemente; a linha continua com o status "Completed", apenas riscada.
- **Proposed Change:** Abrir `ConfirmDialog` (com `danger`) antes de `evolution.remove`,
  com o texto da mensagem truncado e o botão "Delete for everyone" como ação explícita.
  Para Downloads, mover o arquivo para a lixeira do sistema (crate `trash`, já comum em
  Tauri) em vez de `remove_file`, confirmar no diálogo e trocar o status da linha para
  "Deleted".
- **User Benefit:** evita perda acidental de mensagens e arquivos; o usuário entende o que
  aconteceu.
- **Risks:** um passo extra para quem apaga com frequência; a lixeira não existe em todos
  os ambientes (Flatpak precisa do portal) — manter `remove_file` como fallback
  explicitamente confirmado.
- **Implementation Notes:** reutilizar `src/lib/components/ConfirmDialog.svelte`; o novo
  status exige um valor em `DownloadStatus` ou um campo derivado de `file_exists`; registrar
  no `CHANGELOG.md`.
- **Verdict:** Implementar.
- **Estimated Effort:** S (confirmação) + M (lixeira e status).

#### UIUX-002 — Composer do chat Evolution inalcançável no tamanho mínimo de janela e em 200 %

- **Category:** Usability
- **Priority Rationale:** bloqueia a tarefa principal (enviar mensagem) em configurações
  que o próprio app permite (janela mínima 480x400; escala até 200 %, opção de
  acessibilidade).
- **Confidence:** Alta — observado no app real.
- **Evidence:** `.ideation/uiux-screenshots/evo-conversation-480x400.png` (painel da conversa
  com ~116 px; sem campo de texto nem botão enviar),
  `.ideation/uiux-screenshots/evo-conversation-scale200-1000x700.png` (nome do contato some,
  campo de texto com largura zero, enviar cortado na borda),
  `.ideation/uiux-screenshots/evo-chatlist-scale200-1000x700.png` (estado vazio cortado),
  `.ideation/uiux-screenshots/evo-conversation-640x480.png` (placeholder truncado em "Type a").
  Código: `src/lib/evolution/ChatView.svelte:379`, `src-tauri/src/app/window.rs:24`,
  `src/lib/settings/pages/Appearance.svelte:25-28`.
- **Affected Journey/Screens:** abrir conversa e responder em janela estreita ou com escala
  ampliada.
- **Affected Code:** `.wa { grid-template-columns: minmax(300px, 30%) 1fr }` sem media
  query; `min_inner_size(480.0, 400.0)`; escalas de 50 % a 200 %.
- **Current State:** a lista de conversas nunca encolhe abaixo de 300 px; a coluna da
  conversa recebe o resto, mesmo que seja ~100 px.
- **Proposed Change:** Adotar layout de painel único abaixo de ~720 px CSS de largura do
  conteúdo: com conversa aberta, mostrar só a conversa com um botão "voltar" no cabeçalho;
  sem conversa, mostrar só a lista. Dar ao composer `min-width` suficiente para o campo e o
  botão enviar.
- **User Benefit:** quem usa janela lado a lado, tela pequena ou escala ampliada consegue
  ler e responder.
- **Risks:** novo estado de navegação (lista ↔ conversa) precisa preservar rascunho e
  posição de rolagem.
- **Implementation Notes:** usar container query ou `ResizeObserver` no `.wa` (a largura
  útil depende da sidebar e da escala, não da janela); o estado `selected` já existe em
  `ChatView.svelte`.
- **Verdict:** Implementar.
- **Estimated Effort:** M.

#### UIUX-003 — Diálogos modais não movem nem prendem o foco; o fundo continua interativo

- **Category:** Accessibility
- **Priority Rationale:** WCAG 2.4.3 (ordem do foco) e 2.1.2; no onboarding de primeiro uso,
  o Tab leva a controles por trás do modal e Enter abre o menu nativo "File" (com "Quit")
  enquanto a configuração inicial está aberta.
- **Confidence:** Alta — app real e harness.
- **Evidence:** `.ideation/uiux-captures/keyboard-sequences.txt` (K1–K4);
  `.ideation/uiux-screenshots/onboarding-tab-escape-behind2-1000x700.png`,
  `.ideation/uiux-screenshots/onboarding-behind-file-menu-open-1000x700.png`,
  `.ideation/uiux-screenshots/shortcuts-dialog-1000x700.png` (anel de foco no botão Downloads
  da sidebar, atrás do backdrop), `.ideation/uiux-screenshots/open-chat-dialog-chromium-1000x700.png`,
  `.ideation/uiux-snapshots/onboarding-basics-1000x700.txt` (todo o fundo exposto e
  focável). Código: `src/lib/onboarding/Onboarding.svelte:75`,
  `src/lib/shortcuts/ShortcutsDialog.svelte:60`, `src/lib/chat/OpenChatDialog.svelte:75`,
  `src/lib/evolution/SetupDialog.svelte:96`, `src/lib/components/ConfirmDialog.svelte:35`,
  `src/routes/+page.svelte:356`; nenhum uso de `inert` ou armadilha de foco em `src/`.
- **Affected Journey/Screens:** 17 elementos `role="dialog"`/`"alertdialog"` em 15 arquivos, todos com
  `aria-modal="true"` (onboarding, atalhos, enviar para número, Evolution Go, confirmações,
  downloads, dicionários, relatórios, customizações, atualização, falha).
- **Affected Code:** `+page.svelte` renderiza `.window` antes dos diálogos e não o marca
  como `inert`.
- **Current State:** onboarding e atalhos abrem com o foco em `BODY` ou no controle de fundo;
  "Send message to number" foca o campo certo, mas o terceiro Tab sai para `BODY` e depois
  para o menu bar.
- **Proposed Change:** Criar um componente `Modal.svelte` que: aplica `inert` ao `.window`
  e às janelas irmãs enquanto aberto; foca o primeiro campo ou a ação segura ao abrir;
  mantém Tab/Shift+Tab dentro do diálogo; fecha com Escape quando cancelável; devolve o
  foco ao elemento que o abriu. Migrar todos os diálogos para ele.
- **User Benefit:** usuários de teclado e leitor de tela ficam no contexto do diálogo e
  não acionam o app por trás dele.
- **Risks:** os menus nativos (muda) não respeitam `inert`; desabilitar os itens do menu
  bar enquanto um modal estiver aberto exige coordenar com `MenuBar.svelte`.
- **Implementation Notes:** `layout.setOverlay(true)` já é chamado pelos diálogos; o
  primitivo pode centralizar isso também.
- **Verdict:** Implementar.
- **Estimated Effort:** M.

#### UIUX-004 — Controles visíveis só no hover: foco invisível e ações inalcançáveis pelo teclado

- **Category:** Accessibility
- **Priority Rationale:** WCAG 2.4.7 (foco visível) e 2.1.1 (teclado). No chat, cada
  mensagem adiciona dois pontos de Tab invisíveis; em Downloads, abrir pasta e apagar não
  existem para o teclado nem na árvore de acessibilidade.
- **Confidence:** Alta.
- **Evidence:** `.ideation/uiux-screenshots/evo-conversation-tab13-focus-hidden-1000x700.png`,
  `.ideation/uiux-screenshots/evo-conversation-tab13-enter-menu-1000x700.png`;
  `.ideation/uiux-captures/keyboard-sequences.txt` K5 (foco invisível, Enter abre menu),
  K6 (38 Tabs da conversa selecionada até o composer numa conversa de 11 mensagens), K7
  (Downloads pula 📂 e 🗑); `.ideation/uiux-captures/contrast-and-target-measurements.txt`
  (`.more`/`.react` com `opacity: 0` em `:focus-visible`);
  `.ideation/uiux-snapshots/downloads-list-1000x700.txt` (botões ausentes);
  `.ideation/uiux-screenshots/downloads-list-hover-chromium-1000x700.png`. Código:
  `src/lib/evolution/Conversation.svelte:462`, `:534`, `:874`, `:885-888`, `:899`;
  `src/lib/downloads/DownloadsView.svelte:88`, `:91`, `:178-185`.
- **Affected Journey/Screens:** responder/reagir/copiar/apagar mensagem pelo teclado; abrir
  pasta ou apagar um download.
- **Affected Code:** `.more`/`.react { opacity: 0 }` revelados só por `.row:hover`;
  `.row-actions { visibility: hidden }` revelado só por `li:hover`; botões só com `title`
  e emoji como conteúdo.
- **Current State:** o foco cai em botões invisíveis; Downloads não oferece caminho de
  teclado visível (só o menu de contexto nativo).
- **Proposed Change:** Revelar os controles também com `.row:focus-within` e
  `li:focus-within`; trocar `visibility: hidden` por `opacity: 0` com revelação em
  `:focus-within`; dar `aria-label` traduzido aos botões 📂/🗑 incluindo o nome do arquivo.
  No chat, transformar a lista de mensagens num único ponto de Tab com navegação por setas
  (roving tabindex) e abrir as ações com Enter ou com a tecla de menu de contexto, em vez
  de dois botões por mensagem na ordem de Tab.
- **User Benefit:** quem usa teclado vê onde está, alcança todas as ações e chega ao
  composer sem atravessar dezenas de botões.
- **Risks:** roving tabindex muda o modelo de navegação; documentar no diálogo de atalhos.
- **Implementation Notes:** o atalho de foco do composer já existe implicitamente
  (`composer?.focus()` em `Conversation.svelte:303`, `:346`); considerar também um atalho
  explícito.
- **Verdict:** Implementar.
- **Estimated Effort:** S (`:focus-within` e rótulos) + M (roving tabindex).

#### UIUX-005 — Contraste abaixo de 4,5:1 em texto essencial no tema claro

- **Category:** Accessibility
- **Priority Rationale:** WCAG 1.4.3/1.4.11 falham em conteúdo central — links, nomes de
  remetentes, contadores de não lidas, botões primários; o autor da citação em amarelo
  sobre verde (1,15:1) é praticamente ilegível.
- **Confidence:** Alta — medido por estilo computado; axe-core confirma 6 nós (serious).
- **Evidence:** `.ideation/uiux-captures/contrast-and-target-measurements.txt`,
  `.ideation/uiux-captures/axe-evo-conversation-1000x700.txt`,
  `.ideation/uiux-screenshots/evo-conversation-states-chromium-1000x700.png` (autor "Ana
  Souza" quase invisível na citação), `.ideation/uiux-screenshots/evo-chatlist-idle-1000x700.png`,
  `.ideation/uiux-screenshots/settings-accounts-1000x700.png`,
  `.ideation/uiux-screenshots/panel-default-320x420.png`. Código:
  `src/lib/evolution/format.ts:94`, `src/lib/evolution/Conversation.svelte:466-473`,
  `:943-945`, `src/lib/evolution/ChatView.svelte:359`, `:364-367`, `:625-626`, `:664-665`,
  `src/app.css:5`, `:50-54`, `src/lib/settings/SettingsView.svelte:107-110`,
  `src/routes/+page.svelte:590-598`.
- **Affected Journey/Screens:** lista de conversas, conversa (links, citações, grupos,
  recibos), Configurações, painel.
- **Affected Code / Current State (tema claro):**

  | Elemento | Cores | Razão | Mínimo |
  |---|---|---|---|
  | Autor da citação (`SENDER_COLORS` #ffd279 em #cef0c8) | 12–13 px 600 | 1,15 | 4,5 |
  | Paleta de remetentes em balão branco (8 cores) | 13 px | 1,42–4,00 | 4,5 |
  | Recibo "lido" (#53bdeb em #d9fdd3) | ícone | 1,92 | 3,0 |
  | Link em balão (#53bdeb em #fff) | 14,2 px | 2,13 | 4,5 |
  | Item ativo das Configurações (#00a884 em #eae9e7) | 14 px 600 | 2,50 | 4,5 |
  | Horário/contador de não lidas (#1daa61 / branco em #1daa61) | 12 px 600 | 3,01 | 4,5 |
  | Botão primário, segmento "Enabled", badges do painel (branco em #00a884) | 11–16 px | 3,03 | 4,5 |
  | Contador do chip "Unread" (#828181) | 12 px | 3,87 | 4,5 |
  | Hora em balão enviado (#667781 em #d9fdd3) | 11 px | 4,19 | 4,5 |
  | Badge da sidebar (branco em #e53935) | 11 px | 4,23 | 4,5 |

  No tema escuro, links, citações, recibos e horários passam; botões primários (3,03) não.
- **Proposed Change:** Definir variantes de tema claro para `--wa-green` (texto), link,
  recibo "lido" e paleta de remetentes (tons escurecidos com ≥ 4,5:1 sobre #fff e
  #d9fdd3); criar `--accent-strong` (≈ #007a5e, 5,3:1 com branco) para fundos de botões
  primários/segmentos/badges e para o texto do item ativo; escurecer o vermelho do badge
  (≈ #c62828).
- **User Benefit:** leitura confiável para baixa visão, telas com brilho alto e luz solar.
- **Risks:** afasta-se das cores exatas do WhatsApp; validar a identidade visual com o
  mantenedor.
- **Implementation Notes:** `senderColor` deve receber o tema ou retornar um índice
  mapeado para variáveis CSS (`--wa-sender-0…7`) definidas por tema; adicionar um teste
  Vitest que verifica a razão mínima da paleta clara.
- **Verdict:** Implementar.
- **Estimated Effort:** S–M.

### Medium

#### UIUX-006 — Configurações, grade e onboarding quebram em larguras compactas

- **Category:** Usability
- **Priority Rationale:** a janela mínima permitida (480x400) e a escala 200 % produzem
  conteúdo cortado, sobreposto ou sem orientação; não bloqueia a tarefa principal como o
  UIUX-002, mas afeta toda a configuração.
- **Confidence:** Alta.
- **Evidence:** `.ideation/uiux-screenshots/settings-accounts-480x400.png` ("+ Add account"
  cortado), `.ideation/uiux-screenshots/settings-accounts-640x480.png` (nome "U…", botões
  quebrados), `.ideation/uiux-screenshots/settings-appearance-scale200-1000x700.png`
  (coluna de conteúdo estreita, switch sobre o texto),
  `.ideation/uiux-screenshots/grid-overview-480x400.png` (cartões transbordam o contêiner),
  `.ideation/uiux-screenshots/onboarding-background-640x480.png` e
  `.ideation/uiux-screenshots/onboarding-background-480x400.png` (navegação de passos
  oculta, sem "passo N de 6"). Código: `src/lib/settings/SettingsView.svelte:59-62`,
  `src/lib/accounts/AccountsOverview.svelte:106`, `src/lib/onboarding/Onboarding.svelte:362-366`.
- **Affected Journey/Screens:** Configurações (todas as páginas), visão geral de contas,
  onboarding.
- **Affected Code:** `aside { width: 260px }` fixo; `repeat(var(--columns), minmax(220px, 1fr))`
  com `grid_cols` padrão 2; `@media (max-width: 640px) { nav { display: none } }`.
- **Current State:** em 480 px sobram ~156 px para o conteúdo das Configurações; a grade
  exige 2×220 px; o onboarding esconde a navegação sem substituto.
- **Proposed Change:** Abaixo de ~720 px de conteúdo, recolher a navegação das
  Configurações num seletor no topo (ou lista → página com "voltar"); calcular colunas da
  grade com `repeat(auto-fill, minmax(220px, 1fr))` limitado por `grid_cols`; no
  onboarding compacto, mostrar "Passo N de 6 — título" acima do conteúdo.
- **User Benefit:** configurações utilizáveis em janelas lado a lado e com escala ampliada.
- **Risks:** `grid_cols` deixa de ser exato em larguras pequenas; documentar como máximo.
- **Implementation Notes:** compartilhar breakpoints com o UIUX-002 (ver S-2).
- **Verdict:** Implementar.
- **Estimated Effort:** M.

#### UIUX-007 — Interface pt-BR mistura inglês: 21 % dos textos sem tradução e telas inteiras fora do i18n

- **Category:** Usability
- **Priority Rationale:** o público principal (documentação em pt-BR) vê o chat Evolution,
  o painel e a confirmação de saída em inglês.
- **Confidence:** Alta — runtime em pt_BR e varredura do catálogo.
- **Evidence:** `.ideation/uiux-screenshots/evo-conversation-ptbr-chromium-1000x700.png`
  ("Todos" traduzido, mas "Chats", "Search or start a new chat", "Unread", "Groups",
  "Archived", "Document", "edited", "Type a message" em inglês),
  `.ideation/uiux-screenshots/close-confirm-ptbr-chromium-1000x700.png`,
  `.ideation/uiux-screenshots/panel-ptbr-chromium-320x420.png`,
  `.ideation/uiux-snapshots/panel-ptbr-320x420.txt`,
  `.ideation/uiux-captures/i18n-ptbr-missing-msgids.txt` (157 de 735 msgids sem pt_BR).
  Código: `src/routes/panel/+page.svelte:61-108` (sem import de `t`),
  `src/routes/+page.svelte:278`, `:339`, `:357-363`.
- **Affected Journey/Screens:** chat Evolution, configuração Evolution, painel de acesso
  rápido, confirmação de fechamento, placeholder "No enabled accounts.", título "Do not
  disturb" do indicador da sidebar.
- **Affected Code:** strings literais fora de `t()`; `po/pt_BR.po` sem entradas para textos
  novos (não há extrator, conforme `AGENTS.md`).
- **Current State:** mistura de idiomas na mesma tela; o painel mostra "Account 1",
  "Quick access", "🔊 Mute" em qualquer locale.
- **Proposed Change:** Envolver em `t()` todas as strings de `panel/+page.svelte` e da
  confirmação de fechamento; adicionar as 157 entradas a `po/pt_BR.po` e rodar `pnpm i18n`;
  adicionar um extrator de msgids com um gate na CI que falha quando um msgid novo não
  existe no `.pot`.
- **User Benefit:** interface consistente no idioma do usuário.
- **Risks:** as traduções dos outros 17 idiomas continuam incompletas até haver
  contribuição; o gate deve exigir o `.pot`, não todas as traduções.
- **Implementation Notes:** sobrepõe-se ao item CI-005/CI-005B já identificado na análise
  `IDEATION_CODE_IMPROVEMENTS` (memória do projeto); coordenar para não duplicar.
- **Verdict:** Implementar.
- **Estimated Effort:** M.

#### UIUX-008 — Erros do servidor aparecem crus, em inglês e sem orientação de recuperação

- **Category:** State Handling
- **Priority Rationale:** mensagens como "the server returned 503: service unavailable" não
  dizem o que fazer; o erro de credencial não aponta o campo.
- **Confidence:** Alta.
- **Evidence:** `.ideation/uiux-screenshots/evo-offline-banner-1000x700.png`,
  `.ideation/uiux-screenshots/evo-setup-error-1000x700.png` ("the server refused the
  credentials"), `.ideation/uiux-screenshots/evo-conversation-states-chromium-1000x700.png`
  ("this media is no longer on WhatsApp's servers; open it on your phone"). Código:
  `src-tauri/src/evolution/client.rs:56-77` (`#[error(...)]` em inglês minúsculo, sem
  `tr()`), `src/lib/evolution/Conversation.svelte:104-106` (`errorMessage` repassa
  `.message`).
- **Affected Journey/Screens:** banner offline da lista de conversas, diálogo Evolution Go,
  mídia expirada, falhas de envio.
- **Affected Code:** `EvoError` atravessa `?` até `CommandError` sem tradução, enquanto
  erros de nível de comando usam `tr()` (`commands/evolution.rs:745`).
- **Current State:** texto técnico, iniciado em minúscula, sem ação sugerida; no diálogo,
  o erro fica abaixo do formulário sem `aria-invalid`/`aria-describedby` no campo da chave.
- **Proposed Change:** Mapear cada variante de `EvoError` para uma mensagem traduzida com
  causa e ação ("Não foi possível falar com o servidor (503). O FastZap tenta de novo
  automaticamente; toque em Tentar de novo para forçar."); manter o detalhe técnico num
  "Detalhes" expansível; no diálogo, marcar o campo "Admin key" com `aria-invalid` e
  ligar a mensagem por `aria-describedby`.
- **User Benefit:** o usuário entende se o problema é dele (chave), do servidor ou da rede.
- **Risks:** mensagens do servidor já são sanitizadas; o detalhe técnico não pode vazar
  chave/URL com segredo (regra de `AGENTS.md`).
- **Implementation Notes:** um `fn user_message(&EvoError) -> String` com `tr()` no módulo
  de comandos evita mudar o `Display` usado em logs.
- **Verdict:** Implementar.
- **Estimated Effort:** S–M.

#### UIUX-009 — A mesma conta tem quatro nomes e nomes acessíveis errados

- **Category:** Accessibility
- **Priority Rationale:** WCAG 4.1.2 e 2.5.3 (rótulo no nome): a conta padrão é anunciada
  como "3" na sidebar; o cartão da grade diz "Unnamed account" mas se chama "Account";
  usuários de voz/leitor de tela não conseguem identificar contas.
- **Confidence:** Alta — árvore de acessibilidade do Chromium; nomes visuais confirmados no
  app real.
- **Evidence:** `.ideation/uiux-snapshots/shell-evo-idle-1000x700.txt` (`button "3"`,
  `button "▦"`, `button "⚙"`, `button "Audit Evolution"` sem o "9"),
  `.ideation/uiux-snapshots/grid-overview-1000x700.txt` (`button "Account"` com texto
  "Unnamed account"), `.ideation/uiux-snapshots/panel-default-320x420.txt` ("Account 1"),
  `.ideation/uiux-snapshots/settings-accounts-1000x700.txt` (três "Edit", três "Remove",
  três "Account status" sem contexto), `.ideation/uiux-screenshots/panel-default-320x420.png`,
  `.ideation/uiux-screenshots/grid-overview-1000x700.png`,
  `.ideation/uiux-screenshots/panel-new-account-result-1000x700.png` (duas "Unnamed
  account"). Código: `src/routes/+page.svelte:268-269`, `:278`, `:298`, `:299-305`;
  `src/lib/accounts/AccountsOverview.svelte:58`, `:66`; `src/routes/panel/+page.svelte:76`;
  `src/lib/settings/pages/Accounts.svelte:106-109`, `:117`.
- **Affected Journey/Screens:** sidebar, grade, painel, menu "Users", Configurações › Contas.
- **Affected Code:** `aria-label={account.name}` vazio para a conta padrão (o nome cai para
  o texto do badge); três fallbacks diferentes (`t("Account")`, `t("Unnamed account")`,
  `` `Account ${index + 1}` ``); botões de ferramenta só com `title` e glifo.
- **Current State:** nomes divergentes por superfície; contadores de não lidas e "não
  perturbe" ausentes do nome acessível; botões repetidos sem a conta no nome.
- **Proposed Change:** Criar `accountLabel(account, index)` em `src/lib/accounts/` usado
  por sidebar, grade, painel, menu "Users" e Configurações (ex.: "Conta 1" quando sem
  nome); compor o nome acessível com estado ("Audit Evolution, 9 não lidas, não
  perturbe"); dar `aria-label` traduzido a "▦" e "⚙"; incluir o nome da conta nos
  `aria-label` de Edit/Remove/Account status.
- **User Benefit:** contas identificáveis por qualquer pessoa e por tecnologia assistiva.
- **Risks:** nenhum funcional; muda textos visíveis em três superfícies.
- **Implementation Notes:** o rótulo do menu "Users" já usa `Account {number}`
  (`MenuBar.svelte`); alinhar a esse formato traduzido.
- **Verdict:** Implementar.
- **Estimated Effort:** S.

#### UIUX-010 — Estado de entrega e falha só por cor/ícone; "falhou" e "talvez enviada" são iguais

- **Category:** State Handling
- **Priority Rationale:** o usuário não distingue uma mensagem que falhou de uma que talvez
  tenha sido enviada (reenviar pode duplicar); a recuperação está escondida num menu de
  hover; leitores de tela não recebem nenhum estado.
- **Confidence:** Alta.
- **Evidence:** `.ideation/uiux-screenshots/evo-conversation-states-chromium-1000x700.png`
  (mesmo ícone vermelho para `failed` e `unknown`; "lido" e "entregue" diferem só na cor),
  `.ideation/uiux-screenshots/evo-conversation-default-1000x700.png`,
  `.ideation/uiux-snapshots/evo-conversation-1000x700.txt` (nenhum estado, remetente ou
  "você" exposto; mensagens próprias e recebidas idênticas). Código:
  `src/lib/evolution/format.ts:31-32`, `src/lib/evolution/Conversation.svelte:408-410`
  (`title={m.status}` cru), `:562-566`.
- **Affected Journey/Screens:** conversa Evolution.
- **Affected Code:** `statusIcon` mapeia `unknown` e `failed` para `"alert"`, `delivered` e
  `read` para `"checks"`.
- **Current State:** ícone + cor; título com o valor interno em inglês ("failed",
  "unknown"); "Send again" só dentro do menu de ações.
- **Proposed Change:** Exibir abaixo do balão com falha um texto traduzido e um botão
  visível: "Não enviada · Tentar de novo" (`failed`) e "Envio incerto · Verificar no
  celular / Enviar de novo" (`unknown`); usar ícones distintos; expor estado e autoria em
  texto visualmente oculto ("Você, 20:29, lida").
- **User Benefit:** recuperação direta e sem duplicatas acidentais.
- **Risks:** o reenvio de `unknown` continua manual por regra do backend (nunca reenviar
  automaticamente); manter a confirmação explícita.
- **Implementation Notes:** `msgid`s novos para os rótulos de estado; o ícone `clock` já
  cobre `pending`.
- **Verdict:** Implementar.
- **Estimated Effort:** S–M.

#### UIUX-011 — Desligar sidebar e menu bar deixa a janela sem navegação visível

- **Category:** Interaction
- **Priority Rationale:** duas opções independentes em Aparência removem todo acesso visível
  a Configurações, Downloads, grade e troca de contas; a única saída é Ctrl+P (não
  indicado em lugar algum) ou a bandeja.
- **Confidence:** Alta.
- **Evidence:** `.ideation/uiux-screenshots/settings-sidebar-off-1000x700.png`,
  `.ideation/uiux-screenshots/settings-sidebar-menubar-off-1000x700.png`,
  `.ideation/uiux-screenshots/shell-no-sidebar-no-menubar-1000x700.png`,
  `.ideation/uiux-screenshots/shell-no-chrome-evolution-1000x700.png`,
  `.ideation/uiux-screenshots/shell-no-chrome-ctrlp-recovery-1000x700.png`;
  `.ideation/uiux-captures/keyboard-sequences.txt` K8. Código:
  `src/lib/settings/pages/Appearance.svelte:42-53`.
- **Affected Journey/Screens:** Configurações › Aparência › Interface.
- **Affected Code:** dois `SwitchRow` sem regra cruzada.
- **Current State:** o app aceita a combinação sem aviso.
- **Proposed Change:** Ao desligar o segundo dos dois, mostrar um aviso inline com o atalho
  de recuperação ("Use Ctrl+P para abrir as Configurações ou o ícone da bandeja") e, se a
  bandeja estiver desativada, impedir a combinação.
- **User Benefit:** evita que o usuário fique preso numa janela sem saída aparente.
- **Risks:** usuários avançados querem a janela limpa; o aviso não deve bloquear quando há
  bandeja.
- **Implementation Notes:** a bandeja é `settings.appearance.tray_icon`
  (`src-tauri/src/settings/mod.rs:120`, padrão `true`); `patchSettings` ignora chaves
  desconhecidas, então use exatamente esse nome.
- **Verdict:** Implementar.
- **Estimated Effort:** S.

#### UIUX-012 — Menu bar nativo sem aceleradores e com item que não faz nada

- **Category:** Interaction
- **Priority Rationale:** atalhos são a forma principal de navegar (Ctrl+P, Ctrl+G, Ctrl+J,
  Ctrl+M), mas os menus não os mostram; "Show monitoring panel" não reage com o painel
  desativado (padrão).
- **Confidence:** Alta para o comportamento observado; Média para a causa da ausência de
  aceleradores [INFERENCE: os menus são abertos como popup do muda, que no GTK não
  renderiza o rótulo de acelerador sem accel group].
- **Evidence:** `.ideation/uiux-screenshots/menubar-view-open-1000x700.png` (nenhum atalho
  ao lado de "Grid View", "Downloads", "Zoom in"…),
  `.ideation/uiux-screenshots/onboarding-behind-file-menu-open-1000x700.png`. Clique em
  View › "Show monitoring panel" com o padrão `floating_panel.enabled = false`: nenhuma
  janela abriu (observação no app real). Código: `src/lib/components/MenuBar.svelte:43`,
  `:105`, `:110`; `src-tauri/src/app/panel.rs:300-305`; `src-tauri/src/tray/mod.rs:44`, `:54`.
- **Affected Journey/Screens:** menu bar (File, View, Chat, Users, Help).
- **Affected Code:** `accelerator: item.shortcut` é passado, mas não aparece; `show()`
  retorna cedo quando o painel está desativado, enquanto a bandeja desabilita o mesmo item.
- **Current State:** atalhos só descobríveis pelo diálogo de atalhos; item de menu sem
  efeito nem explicação.
- **Proposed Change:** Exibir o atalho no rótulo do item quando o acelerador não for
  renderizado (texto alinhado "Grid View\tCtrl+G" ou equivalente suportado pelo muda);
  desabilitar "Show monitoring panel" quando `floating_panel.enabled` for falso, como a
  bandeja já faz, ou ativar o painel ao escolher o item.
- **User Benefit:** atalhos descobríveis; nenhum comando "morto".
- **Risks:** validar a renderização em Windows/macOS, onde o acelerador nativo já aparece.
- **Implementation Notes:** `MenuBar.svelte` é reconstruído quando as configurações mudam;
  o estado `enabled` do item pode vir de `store.settings`.
- **Verdict:** Implementar.
- **Estimated Effort:** S.

#### UIUX-013 — Menus internos (role="menu") cortados no fim da conversa e sem modelo de teclado

- **Category:** Interaction
- **Priority Rationale:** o menu de ações da última mensagem — a mais provável de ser
  editada ou apagada — abre fora da área visível; Escape não fecha, setas não navegam.
- **Confidence:** Alta.
- **Evidence:** `.ideation/uiux-screenshots/evo-conversation-own-actions-menu-1000x700.png`
  (só uma fresta do menu visível), `.ideation/uiux-screenshots/evo-conversation-own-actions-menu-scrolled-1000x700.png`
  (precisa rolar), `.ideation/uiux-screenshots/evo-conversation-menu-after-escape-1000x700.png`
  (Escape não fecha). Código: `src/lib/evolution/Conversation.svelte:359-363`, `:401`,
  `:543`, `:554`, `:768-773`, `:1097-1100`; `src/lib/evolution/ChatView.svelte:158-160`,
  `:212-216`.
- **Affected Journey/Screens:** menu de ações e de reações das mensagens; menu da lista de
  conversas; menu da conversa.
- **Affected Code:** `.menu { position: absolute; top: 28px }` dentro de `.messages`
  (`overflow-y: auto`); fechamento só por clique fora (`onWindowClick`); o foco não entra
  no menu; botões `.more` sem `aria-expanded`/`aria-haspopup`.
- **Current State:** menus abrem sempre para baixo e são recortados pelo contêiner rolável.
- **Proposed Change:** Criar um `Menu.svelte` compartilhado que: posiciona com
  `position: fixed` e inverte para cima quando não há espaço; move o foco para o primeiro
  item; navega com setas/Home/End; fecha com Escape e Tab devolvendo o foco ao botão;
  expõe `aria-expanded`/`aria-haspopup` no gatilho.
- **User Benefit:** ações sempre visíveis e operáveis por teclado.
- **Risks:** `position: fixed` precisa recalcular em rolagem/redimensionamento; fechar ao
  rolar é aceitável.
- **Implementation Notes:** os três menus seguem o mesmo padrão e podem migrar juntos.
- **Verdict:** Implementar.
- **Estimated Effort:** M.

#### UIUX-014 — "New account" e Ctrl+U criam uma conta imediatamente, sem escolha nem nome

- **Category:** Interaction
- **Priority Rationale:** um clique no painel ou um Ctrl+U acidental cria uma conta
  WhatsApp Web (novo webview e perfil), ocupa uma das 5 vagas e gera mais uma "Unnamed
  account"; Evolution Go só é oferecido em outro botão, dentro das Configurações.
- **Confidence:** Alta.
- **Evidence:** `.ideation/uiux-screenshots/panel-default-320x420.png` → clique em "New
  account" → `.ideation/uiux-screenshots/panel-new-account-result-1000x700.png` ("3 of 5
  accounts configured"; a nova conta fica abaixo da dobra, sem foco nem destaque).
  Código: `src/lib/accounts/actions.ts:7-8`, `src/routes/+page.svelte:177-180`,
  `src/routes/panel/+page.svelte:95-98`, `src/lib/components/MenuBar.svelte:80`,
  `src/lib/settings/pages/Accounts.svelte:78-83`.
- **Affected Journey/Screens:** painel, menu Users, atalho Ctrl+U, Configurações › Contas.
- **Affected Code:** `addAccount()` chama `accounts_create` com `name: ""` e
  `kind: "whatsapp_web"` sem interação.
- **Current State:** paridade com o ZapZap (comentário em `+page.svelte:178`), mas o FastZap
  tem dois tipos de conta e um limite de contas.
- **Proposed Change:** Abrir um diálogo "Adicionar conta" com a escolha WhatsApp Web /
  Evolution Go e um campo de nome opcional; após criar, rolar até o cartão novo, focá-lo e
  destacá-lo brevemente.
- **User Benefit:** sem contas fantasmas; o tipo Evolution Go fica descobrível no mesmo
  fluxo.
- **Risks:** um passo a mais para quem só usa WhatsApp Web; o diálogo pode lembrar a última
  escolha.
- **Implementation Notes:** o diálogo pode reutilizar `SetupDialog.svelte` para o ramo
  Evolution Go.
- **Verdict:** Implementar após decisão de produto (a paridade com o ZapZap é intencional).
- **Estimated Effort:** M.

#### UIUX-015 — Diálogo Evolution Go: servidor pessoal pré-preenchido e botão primário desabilitado sem explicação

- **Category:** Usability
- **Priority Rationale:** o campo de servidor vem com o endereço de um servidor pessoal; um
  usuário que só cola a chave admin envia essa chave para um servidor de terceiros. O
  botão "Add account" fica desabilitado até "Load instances" sem dizer por quê.
- **Confidence:** Alta para o estado da UI; a consequência de envio da chave segue do código
  (`baseUrl` inicia com `DEFAULT_SERVER`) e não foi exercitada contra o servidor real.
- **Evidence:** `.ideation/uiux-screenshots/evo-setup-empty-1000x700.png`,
  `.ideation/uiux-screenshots/evo-setup-instances-1000x700.png`. Código:
  `src/lib/evolution/SetupDialog.svelte:12`, `:15`, `:130`, `:176`.
- **Affected Journey/Screens:** Configurações › Contas › "+ Evolution Go".
- **Affected Code:** `const DEFAULT_SERVER = "https://evolutiongo.diogosrodrigues.com.br"`;
  `disabled={!ready || (instances === null && !account)}`.
- **Current State:** campo pré-preenchido parece um valor padrão do produto; o fluxo exige
  descobrir a ordem "chave → Load instances → escolher → Add account".
- **Proposed Change:** Deixar o endereço vazio com placeholder "https://seu-servidor…";
  numerar os passos no diálogo e mostrar sob o botão desabilitado "Carregue as instâncias
  para continuar".
- **User Benefit:** evita enviar credenciais ao servidor errado e reduz tentativa e erro.
- **Risks:** o mantenedor perde o atalho do próprio servidor; pode ficar num build de
  desenvolvimento ou numa variável de ambiente.
- **Implementation Notes:** nenhuma mudança de persistência; só estado inicial do diálogo.
- **Verdict:** Implementar.
- **Estimated Effort:** S.

### Low

#### UIUX-016 — Indicador de foco removido nos campos de texto principais

- **Category:** Accessibility
- **Priority Rationale:** WCAG 2.4.7; o cursor de texto ainda indica foco, por isso Low.
- **Confidence:** Alta.
- **Evidence:** `.ideation/uiux-captures/contrast-and-target-measurements.txt` (busca e
  composer focados: `outline none`, `box-shadow none`). Código:
  `src/lib/evolution/ChatView.svelte:540`, `:721`, `src/lib/evolution/Conversation.svelte:1245`;
  única regra `:focus-visible` do projeto em `src/lib/accounts/AccountsOverview.svelte:134`.
- **Affected Journey/Screens:** busca de conversas, composer, campo "nova conversa".
- **Affected Code:** `outline: 0` sem substituto no contêiner.
- **Current State:** o contêiner arredondado não muda ao receber foco.
- **Proposed Change:** Adicionar `:focus-within` aos contêineres (`.search`, `.compose`,
  `.new`) com borda ou sombra de 2 px na cor de acento e um estilo global
  `:focus-visible` em `src/app.css` para botões e links.
- **User Benefit:** localização imediata do foco.
- **Risks:** nenhum.
- **Implementation Notes:** usar o `--accent-strong` do UIUX-005 para manter 3:1.
- **Verdict:** Implementar.
- **Estimated Effort:** S.

#### UIUX-017 — Onboarding liga o corretor sem dicionário e não oferece como obter um

- **Category:** Usability
- **Priority Rationale:** o switch "Enable spell checker" vem ligado com "No dictionary
  available"; o gerenciador de dicionários só existe nas Configurações.
- **Confidence:** Alta para a UI; o perfil isolado não tinha dicionários instalados.
- **Evidence:** `.ideation/uiux-screenshots/onboarding-files-1000x700.png`. Código:
  `src/lib/onboarding/Onboarding.svelte:206-213`;
  `src/lib/settings/pages/LanguageDownloads.svelte:117-118` (botão "Manage…").
- **Affected Journey/Screens:** onboarding › Files.
- **Affected Code:** `SwitchRow` sem ação secundária.
- **Current State:** a descrição da seção fala em "Select Hunspell dictionaries", mas não há
  controle para isso.
- **Proposed Change:** Mostrar, quando não houver dicionário, um botão "Baixar dicionário…"
  que abre `DictionaryManager` (com o idioma da interface pré-selecionado).
- **User Benefit:** corretor funcional desde o primeiro uso.
- **Risks:** download no onboarding depende de rede; manter opcional.
- **Implementation Notes:** `DictionaryManager.svelte` já recebe `spell` e callbacks.
- **Verdict:** Implementar.
- **Estimated Effort:** S.

#### UIUX-018 — Confirmação de fechamento destoa do sistema visual

- **Category:** Visual Polish
- **Priority Rationale:** cosmético, mas é o último diálogo que o usuário vê ao sair.
- **Confidence:** Alta.
- **Evidence:** `.ideation/uiux-screenshots/close-confirm-ptbr-chromium-1000x700.png`
  ("No" como botão nativo cinza 33x27 ao lado de "Yes" estilizado),
  `.ideation/uiux-snapshots/close-confirm-ptbr-1000x700.txt`. Código:
  `src/routes/+page.svelte:354-367`.
- **Affected Journey/Screens:** fechar/sair com confirmação ativada.
- **Affected Code:** `<button>` sem classe `.btn`; diálogo próprio em vez de
  `ConfirmDialog`.
- **Current State:** estilos e textos divergentes dos demais diálogos (strings tratadas no
  UIUX-007).
- **Proposed Change:** Substituir pelo `ConfirmDialog` com rótulos de ação explícitos
  ("Cancelar" / "Fechar FastZap") e o checkbox "Não perguntar de novo".
- **User Benefit:** consistência e rótulos que descrevem a ação.
- **Risks:** nenhum.
- **Implementation Notes:** `ConfirmDialog` precisa aceitar um slot para o checkbox.
- **Verdict:** Implementar.
- **Estimated Effort:** S.

#### UIUX-019 — Grade e painel comunicam estado de forma fraca

- **Category:** Visual Polish
- **Priority Rationale:** baixo impacto; a informação existe, mas é ambígua.
- **Confidence:** Alta.
- **Evidence:** `.ideation/uiux-screenshots/grid-overview-1000x700.png` (não lidas como um
  "9" verde solto sob o nome), `.ideation/uiux-snapshots/grid-overview-1000x700.txt` (cartão
  "Trabalho" desativado não exposto como desativado; nomes sem contagem),
  `.ideation/uiux-screenshots/panel-states-chromium-320x420.png`. Código:
  `src/lib/accounts/AccountsOverview.svelte:58`, `:139`.
- **Affected Journey/Screens:** visão geral de contas, painel.
- **Affected Code:** contagem como texto solto; conta desativada só com `opacity: 0.6`.
- **Current State:** "9" sem rótulo; desativada indistinta para leitores de tela.
- **Proposed Change:** Mostrar o badge de não lidas igual ao da sidebar ou o texto "9 não
  lidas"; adicionar o rótulo "Desativada" visível e no nome acessível do cartão.
- **User Benefit:** leitura rápida do estado de cada conta.
- **Risks:** nenhum.
- **Implementation Notes:** combina com `accountLabel` do UIUX-009.
- **Verdict:** Implementar.
- **Estimated Effort:** S.

#### UIUX-020 — "Remove" desabilitado na conta padrão explica o motivo só no tooltip

- **Category:** Usability
- **Priority Rationale:** botões desabilitados não recebem foco e o tooltip não aparece por
  teclado; o motivo fica invisível para muitos usuários.
- **Confidence:** Alta.
- **Evidence:** `.ideation/uiux-screenshots/settings-accounts-1000x700.png` (botão "Remove"
  esmaecido na primeira conta), `.ideation/uiux-snapshots/settings-accounts-1000x700.txt`
  (`button "Remove" [disabled]`). Código: `src/lib/settings/pages/Accounts.svelte:106-109`.
- **Affected Journey/Screens:** Configurações › Contas.
- **Affected Code:** `title={t("The default account cannot be removed.")}` num botão
  `disabled`.
- **Current State:** sem explicação visível.
- **Proposed Change:** Remover o botão na conta padrão e mostrar a nota "Conta padrão — não
  pode ser removida" ao lado do nome.
- **User Benefit:** expectativa clara sem tentativa frustrada.
- **Risks:** nenhum.
- **Implementation Notes:** `DEFAULT_ACCOUNT` já identifica a conta.
- **Verdict:** Implementar.
- **Estimated Effort:** S.

## Structural Recommendations

- **S-1 — Primitivo de diálogo modal.** Um `Modal.svelte` responsável por `inert` no fundo,
  foco inicial, armadilha de foco, Escape, restauração de foco e `layout.setOverlay`.
  Migrar os 17 diálogos. Resolve UIUX-003 e simplifica UIUX-001, UIUX-014 e UIUX-018.
- **S-2 — Contrato responsivo.** Breakpoints únicos baseados na largura útil do conteúdo
  (não da janela), aplicados por container query: chat em painel único, Configurações com
  navegação recolhida, grade com colunas automáticas, onboarding com indicador de passo.
  Testar sempre em 480x400 e em 200 %. Resolve UIUX-002 e UIUX-006.
- **S-3 — Tokens de cor acessíveis por tema.** Separar "cor de marca" de "cor de texto
  sobre fundo"; definir variantes claras/escuras para acento, verde de não lidas, link,
  recibo e paleta de remetentes, com um teste que valida as razões mínimas. Resolve
  UIUX-005 e sustenta UIUX-016.
- **S-4 — Primitivo de menu e política de controles por hover.** Um `Menu.svelte` com
  teclado e posicionamento na viewport, e a regra "todo controle revelado por `:hover`
  também é revelado por `:focus-within`". Resolve UIUX-004 e UIUX-013.
- **S-5 — Cobertura de i18n verificada na CI.** Extrator de msgids para `.pot`, gate que
  falha com string nova fora do `.pot`, e proibição de texto literal em `.svelte` fora de
  `t()` (lint). Resolve a causa do UIUX-007 e evita regressões de UIUX-008.

## Summary

| Categoria | High | Medium | Low | Total |
|---|---|---|---|---|
| Usability | 1 | 3 | 2 | 6 |
| Accessibility | 3 | 1 | 1 | 5 |
| Performance Perception | 0 | 0 | 0 | 0 |
| Visual Polish | 0 | 0 | 2 | 2 |
| Interaction | 1 | 4 | 0 | 5 |
| State Handling | 0 | 2 | 0 | 2 |
| **Total de findings** | **5** | **10** | **5** | **20** |
| Structural | — | — | — | 5 |

- Findings com evidência visual ou de runtime: **20/20**.
- Findings só por código (`[CODE-ONLY]`): **0**.
- Superfícies não inspecionadas: **11** (ver L9).
- Nenhum problema de percepção de desempenho foi observado nos fluxos exercitados (o mock
  responde localmente; latência real de servidor não foi avaliada).
