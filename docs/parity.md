# Paridade com o ZapZap

Referência: ZapZap 7.5.2 (`~/dev/zapzap`). Cada linha indica o módulo de
origem, o módulo equivalente no FastZap e diferenças intencionais.

| Funcionalidade | ZapZap | FastZap | Observações |
|---|---|---|---|
| Instância única e reencaminhamento de links | `app/single_application.py` | `tauri-plugin-single-instance` + `app::lifecycle` | `fastzap://restart` substitui `zapzap://restart` |
| D-Bus `org.freedesktop.Application` | `app/desktop_application_dbus.py` | `app::dbus` | só no Flatpak |
| CLI `--setSettings`, `--wayland`, `--hideStart` | `app/startup_options.py` | `app::cli` | mesmos nomes |
| `SIGTERM` → saída normal | `app/unix_signal_bridge.py` | `app::lifecycle` | |
| Geometria, fechar para bandeja, confirmação | `app/window_lifecycle.py` | `app::lifecycle` + `tauri-plugin-window-state` | `SIGINT` também encerra normalmente |
| Barra de título própria (CSR) | `ui/components/client_side_window*` | `src/lib/components/TitleBar.svelte` | mesmas opções `csr_*`, aplicadas sem reiniciar; moldura opaca de 4 px em vez da margem translúcida com cantos arredondados; novo `appearance.title_bar` ("Show title bar") esconde qualquer barra, nativa ou própria |
| Contas, limite, ativação, avatar, UA, DND, zoom | `features/accounts` | `accounts` + página Contas | tipo de conta novo: Evolution Go |
| Sidebar, grade, bloqueio, atualização | `features/browser/shell` | `src/routes/+page.svelte` + `src/lib/accounts/AccountsOverview.svelte` | a grade é a visão geral de contas (cartões, sem webviews) e inclui contas Evolution Go |
| Barra de menus e escala da interface | `ui/main_window`, `appearance.scale` | `MenuBar.svelte` (menus nativos) + zoom das webviews | |
| Páginas de configurações | `features/settings` | `src/lib/settings/` | mesma ordem e textos |
| Painel flutuante de monitoramento | `ui/components/floating_monitoring_panel.py` | `app::panel` + rota `/panel` (janela `panel`) | posição e travamento só fora do Wayland, como no ZapZap |
| Seletor de contas integrado | `theme_controller.js` (QuickAccounts) | `webview/scripts/quick_accounts.js` | |
| Contador de não lidas pelo título | `web_view.py::_on_title_changed` | `webview/scripts/bridge.js` | |
| Allowlist de navegação e popups | `PopupRoutingPage` | `webview::navigation` | |
| Deep links `whatsapp:` / wa.me | `deeplink.py` | `webview::deeplink` | |
| Chat por número (`Ctrl+M`) | `open_chat.py`, `send_message_to_number_dialog.py` | `webview::deeplink` + `src/lib/chat/` | |
| Colar como texto e tabelas de planilha | `clipboard_table.py`, `web_view.py` | `webview::clipboard` | |
| Anexar arquivos por arraste | `web_view.py` | drag-and-drop HTML no WhatsApp Web; eventos Tauri + `Conversation.svelte` no Evolution Go | Evolution: vários arquivos, remoção antes de confirmar, legenda opcional e limite de 100 MB por arquivo; sem reenvio automático |
| Menu de contexto | `web_view.py` | `spellcheck::linux` (sinal `context-menu` do WebKitGTK) | Linux; remove navegação/abrir link/inspecionar e acrescenta "Verificar ortografia" e "Idiomas…"; rótulos padrão vêm do WebKit no idioma do sistema |
| CSS/JS personalizados | `features/customizations` | `customizations` + página Customizações avançadas | mesma árvore de arquivos; CSS é reaplicado nas páginas abertas ao salvar |
| Corretor ortográfico | `features/dictionaries` (`.bdic`) | `spellcheck` + `src/lib/spellcheck/` | **formato Hunspell** (`.dic/.aff`) do `wooorm/dictionaries` em commit fixo; `.bdic` não é importado; Linux |
| Downloads e histórico (`Ctrl+J`) | `features/downloads` | `downloads` + `src/lib/downloads/` | decisão após o término do download; mídias do chat Evolution Go seguem o mesmo fluxo e histórico; ver lacunas |
| Notificações (portal, D-Bus, Windows, macOS) | `features/notifications` | `notifications` | |
| Bandeja, temas de ícone, contador, badge | `features/tray` | `tray` | |
| Atalhos | `features/shortcuts` | `bridge.js` (páginas) + `src/lib/shell/shortcuts.ts` (UI local) + `src/lib/shortcuts/` | diálogo "Atalhos de teclado"; os dois mapeamentos devem ficar iguais |
| Iniciar com o sistema | `features/startup` | `autostart` | portal `RequestBackground` no Flatpak; entrada XDG nomeada pelo App ID nos demais Linux |
| Permissões (câmera, microfone etc.) | `features/permissions` | `permissions` | Linux; ver lacunas |
| Proxy e proxy estrito | `core/environment/proxy_manager.py` | `proxy` | proxy estrito via bloqueio de WebRTC não proxiado |
| Escudo WebRTC | `webrtc_shield.js` | `webview/scripts/webrtc_shield.js` | |
| Correção de `Ctrl+←/→` | `ctrl_arrow_visual_navigation_fix.js` | idem | |
| Clique em PDF | `pdf_click_controller.js` | idem | |
| Lista de chats redimensionável | `resizable_chat_list.js` | idem | |
| Tema auto/claro/escuro | `core/theme`, `theme_controller.js` | `theme` + script portado | `auto` segue o portal `color-scheme` |
| Idioma da interface | gettext `po/` | `i18n` (catálogos JSON gerados dos `.po`) | |
| Verificação de atualizações | `core/update_checker.py` | `update` | mesma política por empacotamento; diálogo em vez de popover; download pela página de releases |
| Relatórios de problemas | `core/reporting`, `features/reporting` | `reporting` + `src/lib/reporting/` | sem `faulthandler`/zip de crash: logs são os avisos da sessão atual; ver lacunas |
| Doação e lembrete | `features/donation` | — | removido: o FastZap não pede doações |
| Onboarding | `features/initial_setup` | `src/lib/onboarding/` | o idioma é aplicado ao concluir; seção Flatpak mostra o comando `flatpak override` sem link para o Flatseal |
| Desempenho | `core/config/settings/performance.py` | `settings::performance` | ver lacunas |
| Empacotamento | `.github/packaging`, `tools/com.rtosta.zapzap.yaml`, `share/` | `packaging/`, `.github/workflows/build-*.yml` | deb, rpm, AppImage, Flatpak (GNOME 50), snap, Windows NSIS/MSI (x86_64; arm64 só NSIS) e dmg universal; sem COPR, sem Flathub (manifesto baixa dependências pela rede) e macOS sem notarização |
| Conta Evolution Go (novo) | — | `evolution` + `src/lib/evolution/` | chat nativo sobre uma instância Evolution Go: pareamento por QR, código ou passkey; texto, mídia, respostas, reações, edição, exclusão, recibos e digitação; ver lacunas |

## Lacunas conhecidas

- **Flags do Chromium/QtWebEngine** (processo único, processo por site, GC
  previsível, limite de heap JS, tipo/tamanho de cache, flags extras) não têm
  equivalente no WebKitGTK/WebView2/WKWebView. As preferências são importadas
  e preservadas, mas só `disable_gpu`/`software_rendering` (política de
  aceleração de hardware), `persistent_cookies` e `disable_animations` têm
  efeito.
- **Sessão do WhatsApp Web**: perfis do QtWebEngine não são migráveis; após a
  importação cada conta precisa ler o QR de novo.
- **Dicionários `.bdic`**: formato exclusivo do Chromium. O FastZap usa
  Hunspell via Enchant. O catálogo é embutido (sem `manifest.json`/ETag) e
  importar substitui o diálogo de troca por um erro de conflito; a importação
  de pasta não existe. Windows e macOS usam o corretor do sistema sem seleção
  de idiomas pelo app; o menu de contexto também só é ajustado no Linux.
- **Notificações em Windows/macOS** não informam cliques ao app
  (`tauri-plugin-notification`); clicar não abre a conversa. O plugin também
  não devolve identificadores: notificações com a mesma tag não se
  substituem, não são retiradas quando a conversa é lida nem ao sair (o
  backend macOS do ZapZap retirava). O portal não
  recebe o pedido de som silencioso.
- **Proxy estrito** desativa o WebRTC em vez de forçar UDP pelo proxy: o
  WebKit não tem a política `disable_non_proxied_udp` do Chromium. No Linux o
  WebRTC do WebKitGTK é desligado nativamente; nas demais plataformas a API é
  removida de todos os frames por script.
- **Mudo** só tem efeito no Linux (WebKitGTK); WebView2/WKWebView não expõem
  mudo pelo Tauri.
- **Áudio/vídeo chamadas** continuam indisponíveis no WhatsApp Web, como no
  ZapZap.
- **Autenticação de proxy**: as webviews não aceitam usuário/senha de
  proxy; a senha fica no keyring, mas a UI avisa que credenciais não são
  aplicadas.
- **`dont_use_native_dialog`** é importado e exibido, mas o FastZap sempre
  usa os diálogos do sistema.
- **Downloads**: o Tauri não expõe progresso, pausa, retomada nem
  cancelamento. O arquivo é recebido numa pasta temporária e o diálogo de
  download, a pergunta de vários downloads e o "Salvar como" aparecem quando
  ele termina; não há fila de 6 downloads ativos, anel de progresso nem
  popover (o botão da barra lateral abre o histórico). O tipo MIME informado
  pelo servidor não é conhecido, então a abertura automática confere só
  conteúdo e extensão.
- **Permissões em Windows/macOS**: o Tauri não expõe o pedido de permissão
  do WebView2/WKWebView; valem os prompts próprios do sistema e as opções de
  concessão automática não têm efeito.
- **Relatórios**: não há `faulthandler.log`, zips de crash nem rastro de
  diagnóstico do motor. Um pânico vira relatório (o build de release aborta
  logo depois); uma saída sem limpeza só registra o marcador, sem sinal nem
  logs da sessão anterior. Diagnósticos de GPU/VAAPI/Vulkan não são coletados.
- **Contas Evolution Go** reproduzem o layout do WhatsApp Web (lista com
  busca, filtros Tudo/Não lidas/Grupos e pasta Arquivadas; conversa com papel
  de parede, balões com cauda, ticks e reações; compositor com anexo e
  emojis), mas não a barra lateral de status, canais e chamadas, nem gravação
  de áudio. Fixar,
  arquivar, desarquivar e silenciar chats pelo FastZap (a pasta "Arquivadas"
  só espelha o telefone, com até 5 min de atraso), status, chamadas, enquetes, figurinhas
  enviadas, menções, encaminhamento e criação de grupos não existem. Respostas
  a mensagens que o FastZap não guardou chegam sem texto citado; só as
  próprias mensagens podem ser apagadas ou editadas; envios de mídia que
  falham não têm "enviar de novo"; mídia antiga que expirou no CDN do
  WhatsApp não é recuperável (o servidor não tem *media retry*). Histórico antigo depende do sync inicial
  do pareamento ou de um pedido ao telefone ("carregar mensagens antigas");
  o sync inicial só chega uma vez, e uma conversa sem atividade desde então
  só aparece depois de pareamento novo da instância. O FastZap só recebe
  esse sync se, no pareamento, o servidor já gravava o journal
  (`PRIVATE_CHAT_HISTORY`) ou o socket do FastZap estava aberto com
  `HISTORY_SYNC`; uma instância pareada antes disso precisa ser pareada de
  novo para trazer as conversas antigas. Sem `PRIVATE_CHAT_HISTORY` no servidor, eventos perdidos com o
  WebSocket fora não voltam, e um socket que o servidor deixou de abastecer
  só é trocado no próximo erro de conexão; com o journal, mensagens perdidas
  aparecem em até 30 s. O zoom, customizações, corretor e o
  seletor de contas integrado valem só para contas WhatsApp Web; "enviar
  mensagem para número" abre o WhatsApp Web, enquanto o chat nativo tem o
  próprio campo de número. A chave admin do servidor fica no keyring local,
  então o modo serve para uso pessoal num servidor próprio.
