# FastZap

Cliente desktop multi-conta para WhatsApp escrito em Rust + Tauri v2, sucessor
em espírito do [ZapZap](https://github.com/rafatosta/zapzap). Cada conta pode
ser de dois tipos:

- **WhatsApp Web** — o web.whatsapp.com num webview isolado por conta
  (cookies, sessão e cache próprios);
- **Evolution Go** — login numa instância do
  [Evolution Go](https://github.com/EvolutionAPI/evolution-go), com pareamento
  por QR ou código de telefone e uma interface de chat nativa.

## Desenvolvimento

Requisitos: Rust estável, Node 20+, pnpm e as dependências de sistema do Tauri
(no Linux: `webkit2gtk-4.1`, `libsoup-3.0`, `libappindicator`/`ayatana`).

```bash
pnpm install
pnpm tauri dev
```

Documentação técnica em [`docs/`](docs/README.md).

## Licença

GPL-3.0-or-later. Scripts de integração com o WhatsApp Web foram portados do
ZapZap, que usa a mesma licença.
