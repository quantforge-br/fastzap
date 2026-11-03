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
| Barra de título própria (CSR) | `ui/components/client_side_window*` | `src/lib/components/TitleBar.svelte` | mesmas opções `csr_*`, aplicadas sem reiniciar; moldura opaca de 4 px em vez da margem translúcida com cantos arredondados |
| Contas, limite, ativação, avatar, UA, DND, zoom | `features/accounts` | `accounts` + página Contas | tipo de conta novo: Evolution Go |
| Sidebar, grade, bloqueio, doação, atualização | `features/browser/shell` | `src/routes/+page.svelte` | |
| Barra de menus e escala da interface | `ui/main_window`, `appearance.scale` | `MenuBar.svelte` (menus nativos) + zoom das webviews | |
| Páginas de configurações | `features/settings` | `src/lib/settings/` | mesma ordem e textos |
| Painel flutuante de monitoramento | `ui/components/floating_monitoring_panel.py` | `app::panel` + rota `/panel` (janela `panel`) | posição e travamento só fora do Wayland, como no ZapZap |
| Seletor de contas integrado | `theme_controller.js` (QuickAccounts) | `webview/scripts/quick_accounts.js` | |
| Contador de não lidas pelo título | `web_view.py::_on_title_changed` | `webview/scripts/bridge.js` | |
| Allowlist de navegação e popups | `PopupRoutingPage` | `webview::navigation` | |
| Deep links `whatsapp:` / wa.me | `deeplink.py` | `webview::deeplink` | |
| Chat por número (`Ctrl+M`) | `open_chat.py` | `webview::deeplink` + diálogo | |
| Colar como texto e tabelas de planilha | `clipboard_table.py`, `web_view.py` | `webview::clipboard` | |
| Menu de contexto | `web_view.py` | menu padrão do motor; `webview::context_menu` com a fase de corretor | itens de idioma/corretor ainda ausentes |
| CSS/JS personalizados | `features/customizations` | `customizations` + página Customizações avançadas | mesma árvore de arquivos; CSS é reaplicado nas páginas abertas ao salvar |
| Corretor ortográfico | `features/dictionaries` (`.bdic`) | `spellcheck` | **formato Hunspell** (`.dic/.aff`); `.bdic` não é importado |
| Downloads e histórico (`Ctrl+J`) | `features/downloads` | `downloads` + `src/lib/downloads/` | decisão após o término do download; ver lacunas |
| Notificações (portal, D-Bus, Windows, macOS) | `features/notifications` | `notifications` | |
| Bandeja, temas de ícone, contador, badge | `features/tray` | `tray` | |
| Atalhos | `features/shortcuts` | `app::shortcuts` + `bridge.js` | |
| Iniciar com o sistema | `features/startup` | `autostart` | portal `RequestBackground` no Flatpak; entrada XDG nomeada pelo App ID nos demais Linux |
| Permissões (câmera, microfone etc.) | `features/permissions` | `permissions` | Linux; ver lacunas |
| Proxy e proxy estrito | `core/environment/proxy_manager.py` | `proxy` | proxy estrito via bloqueio de WebRTC não proxiado |
| Escudo WebRTC | `webrtc_shield.js` | `webview/scripts/webrtc_shield.js` | |
| Correção de `Ctrl+←/→` | `ctrl_arrow_visual_navigation_fix.js` | idem | |
| Clique em PDF | `pdf_click_controller.js` | idem | |
| Lista de chats redimensionável | `resizable_chat_list.js` | idem | |
| Tema auto/claro/escuro | `core/theme`, `theme_controller.js` | `theme` + script portado | `auto` segue o portal `color-scheme` |
| Idioma da interface | gettext `po/` | `i18n` (catálogos JSON gerados dos `.po`) | |
| Verificação de atualizações | `core/update_checker.py` | `update` | mesma política por empacotamento |
| Relatórios de problemas | `core/reporting`, `features/reporting` | `reporting` | |
| Doação e lembrete | `features/donation` | página Doação | |
| Onboarding | `features/initial_setup` | `src/routes/onboarding` | |
| Desempenho | `core/config/settings/performance.py` | `settings::performance` | ver lacunas |

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
  Hunspell.
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
