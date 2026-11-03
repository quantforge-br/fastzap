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
  payload completo do connect, campos capitalizados, erros não-2xx, timeout
  de envio → resultado desconhecido), decodificação dupla do WebSocket,
  remoção de `instanceToken`, deduplicação e store SQLite.
- **customizations**, **downloads**, **reporting**: regras puras de arquivos,
  histórico e sanitização.

Testes do frontend (Vitest) cobrem funções puras de `src/lib/` (formatação,
i18n, agrupamento de mensagens).

## Verificações manuais

Child webviews, foco, tray, notificações e compositor não são verificáveis em
testes automatizados. Antes de publicar, rode `pnpm tauri dev` e confira:
duas contas WhatsApp Web isoladas, contador de não lidas, clique em notificação,
tray, segunda instância com `xdg-open "whatsapp://send?phone=..."`, downloads,
tema e grade.

Para o Evolution Go, use primeiro o probe somente leitura da skill
`evolutiongo-integration`. Pareamento e envio reais só com conta e destinatário
de teste autorizados.
