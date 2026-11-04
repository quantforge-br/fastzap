# Testes

## Como executar

```bash
pnpm check                      # svelte-check + TypeScript
pnpm test                       # Vitest (src/**/*.test.ts)
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## Suítes

Os testes Rust ficam junto do código (`#[cfg(test)] mod tests`). Cada área
cobre:

- **settings**: defaults iguais aos do ZapZap, leitura de arquivos parciais
  ou corrompidos, migração de versão do JSON.
- **migration**: `ZapZap.conf` com booleanos em texto, chave invertida
  `quit_in_close`, chaves legadas (`system/wayland`,
  `downloads/auto_open_media`), banco `zapzap.db` e mapeamento
  `storage-whats` → conta 1.
- **webview**: allowlist de navegação, deep links, User-Agent por conta.
- **notifications**: política global/por conta, ocultação de nome e texto.
- **evolution**: cliente REST com `wiremock` (escopo admin vs instância,
  payload completo e mesclado do connect, campos capitalizados, erros não-2xx
  sanitizados, timeout de envio → resultado ambíguo, journal com e sem
  cursor, reconnect); WebSocket
  contra um servidor `tokio-tungstenite` local (decodificação dupla, remoção
  de `instanceToken`, filtro de instância, frame de 20 MiB, reconexão,
  renovação que só reconecta depois que o servidor encerrou o socket antigo);
  parser de eventos (texto, resposta, mídia desembrulhada, reação, edição,
  revogação, recibos, HistorySync); store SQLite (deduplicação, outbox,
  recibos que só avançam, falha tardia que não rebaixa mensagem confirmada,
  página do journal aplicada em ordem com o cursor, contagem de mensagens
  que só o journal trouxe, troca de origem que
  limpa o banco, páginas, âncora do history-sync, nomes). O prazo de 65 s
  sem frames do WebSocket e a leitura periódica do journal na sessão não têm
  teste automatizado.
- **customizations**, **downloads**, **reporting**: regras puras de arquivos,
  histórico e sanitização.

Testes do frontend (Vitest) cobrem funções puras de `src/lib/` (formatação,
i18n, validação, `src/lib/evolution/format.ts`).

## Empacotamento

`packaging/common/version.sh --check`, `desktop-file-validate`,
`appstreamcli validate --no-net`, `shellcheck` e `actionlint` validam
metadados, scripts e workflows (os quatro primeiros também no job
`packaging` de `quality.yml`). Os pacotes em si só são verificados
construindo-os; veja [manutenção](maintenance.md#empacotamento).
O instalador Arch é validado com `shellcheck packaging/arch/install.sh` e
`bash -n packaging/arch/PKGBUILD`. Seu smoke é `make install` em Arch Linux:
confira a versão local com `pacman -Q dsr-fastzap-git`, a propriedade de
`/usr/bin/fastzap` e a existência de apenas `fastzap.desktop` no pacote.
`sudo make install` deve ser recusado antes de compilar ou instalar.

## Verificações manuais

Child webviews, foco, tray, notificações e compositor não são verificáveis em
testes automatizados. Antes de publicar, rode `pnpm tauri dev` e confira:
duas contas WhatsApp Web isoladas, contador de não lidas, clique em notificação,
tray, segunda instância com `xdg-open "whatsapp://send?phone=..."`, downloads,
tema e grade (cartões de todas as contas, inclusive desativadas e Evolution
Go; o cartão abre a conta).

Para o Evolution Go, use primeiro o probe somente leitura da skill
`evolutiongo-integration`. Pareamento e envio reais só com conta e destinatário
de teste autorizados.

### Anexos por arraste

- WhatsApp Web: solte arquivos do gerenciador na conversa; confira a prévia
  da página, sem navegação para o arquivo, inclusive depois de trocar de conta.
- Evolution Go: arraste vários arquivos sobre a conversa; confira o destaque,
  nomes e remoção individual. Soltar não envia: Enviar confirma, com legenda
  opcional. Repetir o mesmo caminho não duplica o anexo.
- Soltar sobre a lista de chats, configurações ou fora da conversa não anexa;
  offline, edição e envio em andamento também não aceitam novos anexos.
- Confira escala da interface e HiDPI, saída do arraste sem soltar, pastas,
  arquivo ausente, arquivo vazio e limite de 100 MB. Testes Rust de
  `commands::evolution::upload_tests` cobrem os limites de leitura.
- Com servidor mock, provoque falha no primeiro de vários arquivos: os
  seguintes permanecem pendentes e o tentado não é reenviado automaticamente.
  Troque/feche a conversa durante o envio: os próximos arquivos não saem e
  a conclusão não apaga o rascunho da nova conversa. Fechar/reabrir não pode
  acumular listeners. Nenhum envio real sem destinatário autorizado.
