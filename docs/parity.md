# Paridade com o ZapZap

Referência: ZapZap 7.5.2 (`~/dev/zapzap`). Cada linha indica o módulo de
origem, o módulo equivalente no FastZap e diferenças intencionais.

| Funcionalidade | ZapZap | FastZap | Observações |
|---|---|---|---|
| Instância única e reencaminhamento de links | `app/single_application.py` | `tauri-plugin-single-instance` + `app::instance` | `fastzap://restart` substitui `zapzap://restart` |
| D-Bus `org.freedesktop.Application` | `app/desktop_application_dbus.py` | `app::dbus` | só no Flatpak |
| CLI `--setSettings`, `--wayland`, `--hideStart` | `app/startup_options.py` | `app::cli` | mesmos nomes |
| `SIGTERM` → saída normal | `app/unix_signal_bridge.py` | `app::signals` | |
| Geometria, fechar para bandeja, confirmação | `app/window_lifecycle.py` | `app::window` | |
| Barra de título própria (CSR) | `ui/components/client_side_window*` | `src/lib/components/TitleBar.svelte` | mesmas opções `csr_*` |
| Contas, limite, ativação, avatar, UA, DND, zoom | `features/accounts` | `accounts` + página Contas | tipo de conta novo: Evolution Go |
| Sidebar, grade, bloqueio, doação, atualização | `features/browser/shell` | `src/routes/+page.svelte` | |
| Painel flutuante de monitoramento | `features/browser/shell/floating*` | janela `monitor` | |
| Seletor de contas integrado | `theme_controller.js` (QuickAccounts) | `webview/scripts/quick_accounts.js` | |
| Contador de não lidas pelo título | `web_view.py::_on_title_changed` | `webview/scripts/bridge.js` | |
| Allowlist de navegação e popups | `PopupRoutingPage` | `webview::navigation` | |
| Deep links `whatsapp:` / wa.me | `deeplink.py` | `webview::deeplink` | |
| Chat por número (`Ctrl+M`) | `open_chat.py` | `webview::deeplink` + diálogo | |
| Colar como texto e tabelas de planilha | `clipboard_table.py`, `web_view.py` | `webview::clipboard` | |
| Menu de contexto | `web_view.py` | menu padrão do motor; `webview::context_menu` com a fase de corretor | itens de idioma/corretor ainda ausentes |
| CSS/JS personalizados | `features/customizations` | `customizations` | mesma árvore de arquivos |
| Corretor ortográfico | `features/dictionaries` (`.bdic`) | `spellcheck` | **formato Hunspell** (`.dic/.aff`); `.bdic` não é importado |
| Downloads e histórico (`Ctrl+J`) | `features/downloads` | `downloads` | |
| Notificações (portal, D-Bus, Windows, macOS) | `features/notifications` | `notifications` | |
| Bandeja, temas de ícone, contador, badge | `features/tray` | `tray` | |
| Atalhos | `features/shortcuts` | `app::shortcuts` + `bridge.js` | |
| Iniciar com o sistema | `features/startup` | `autostart` | portal `RequestBackground` no Flatpak |
| Permissões (câmera, microfone etc.) | `features/permissions` | `permissions` | |
| Proxy e proxy estrito | `core/environment/proxy_manager.py` | `proxy` | proxy estrito via bloqueio de WebRTC não proxiado |
| Escudo WebRTC | `webrtc_shield.js` | `webview/scripts/webrtc_shield.js` | |
| Correção de `Ctrl+←/→` | `ctrl_arrow_visual_navigation_fix.js` | idem | |
| Clique em PDF | `pdf_click_controller.js` | idem | |
| Lista de chats redimensionável | `resizable_chat_list.js` | idem | |
| Tema auto/claro/escuro | `core/theme`, `theme_controller.js` | `theme` + script portado | |
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
  (`tauri-plugin-notification`); clicar não abre a conversa. O portal não
  recebe o pedido de som silencioso.
- **Mudo** só tem efeito no Linux (WebKitGTK); WebView2/WKWebView não expõem
  mudo pelo Tauri.
- **Áudio/vídeo chamadas** continuam indisponíveis no WhatsApp Web, como no
  ZapZap.
