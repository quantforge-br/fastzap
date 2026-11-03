# Guia para agentes de manutenção

Antes de alterar o FastZap, leia `CHANGELOG.md`, `docs/README.md`,
`docs/architecture.md`, `docs/maintenance.md`, `docs/COMMIT_CONVENTION.md`,
`docs/parity.md` e a seção pertinente de `docs/testing.md`.

- Preserve comportamento, chaves e valores persistidos; migre explicitamente
  quando uma mudança for inevitável (`src-tauri/src/settings/store.rs`).
- Defaults e nomes de preferências espelham o ZapZap; a importação vive em
  `src-tauri/src/migration/`. Não altere uma chave sem atualizar o importador.
- Segredos (senha do proxy, chave admin e token de instância do Evolution Go)
  vão apenas para o keyring do sistema (`src-tauri/src/secrets.rs`); nunca
  para JSON, logs, eventos enviados ao frontend ou mensagens de erro.
- O webview remoto do WhatsApp Web só pode chamar os comandos listados em
  `src-tauri/capabilities/whatsapp-remote.json`. Não amplie essa lista sem
  justificar no changelog.
- Componentes Svelte reutilizáveis ficam em `src/lib/components/`; páginas não
  definem controles reutilizáveis.
- Mantenha IDs persistidos separados de rótulos traduzidos.
- Registre obrigatoriamente toda mudança na seção marcada `In development` de
  `CHANGELOG.md`.
- Mantenha a versão igual em `package.json`, `src-tauri/Cargo.toml` e
  `src-tauri/tauri.conf.json`.
- Não conclua uma alteração estrutural sem atualizar `docs/architecture.md` e
  `docs/parity.md`.
- Chamadas com efeito colateral ao Evolution Go (criar instância, conectar,
  parear, enviar) só podem ser feitas com autorização explícita do usuário;
  testes usam servidores mock.
- Após alterar código, inclua na entrega uma sugestão de mensagem no padrão
  Conventional Commits (veja `docs/COMMIT_CONVENTION.md`).

Antes da entrega, execute:

```bash
pnpm check
pnpm test
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```
