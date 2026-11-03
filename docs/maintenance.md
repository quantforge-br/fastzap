# Manutenção

## Versões

A versão vive em três lugares que devem ser iguais: `package.json`,
`src-tauri/Cargo.toml` e `src-tauri/tauri.conf.json`. Ao abrir o ciclo seguinte
depois de publicar, derive-a da última tag estável (`X.Y.Z` vira
`X.Y.(Z+1)`); saltos minor ou major são decididos pelo mantenedor.

## Changelog

Toda mudança ou adição — código, documentação, testes, dependências,
empacotamento e workflows — entra na seção marcada `In development` de
`CHANGELOG.md`, agrupada em `Added`, `Changed`, `Fixed` ou `Removed`.

## Preferências persistidas

As preferências ficam em `settings.json` no diretório de configuração do app
(`$XDG_CONFIG_HOME/io.github.quantmind_br.fastzap/` no Linux). O arquivo tem um
campo `version`; mudanças de formato devem acrescentar uma etapa em
`settings::store::migrate` em vez de alterar o significado de um campo.

## Traduções

Ao atualizar `po/*.po`, rode `pnpm i18n` e versione os JSON gerados em
`src/lib/i18n/catalogs/`. Strings novas da UI usam o texto em inglês como
`msgid`.

## Empacotamento

Os manifestos ficam em `packaging/` e os workflows em `.github/workflows/`;
veja [arquitetura](architecture.md#empacotamento). Depois de clonar, rode
`git submodule update --init` (módulos compartilhados do Flathub).

Builds locais:

```bash
pnpm tauri build --bundles deb,rpm,appimage
flatpak-builder --user --install --force-clean --state-dir=.flatpak-builder \
  --install-deps-from=flathub .flatpak-build \
  packaging/flatpak/io.github.quantmind_br.fastzap.yml
packaging/snap/prepare.sh && snapcraft pack
```

Validadores: `packaging/common/version.sh --check`,
`desktop-file-validate packaging/share/applications/*.desktop`,
`appstreamcli validate --no-net packaging/share/metainfo/*.xml` e
`shellcheck packaging/common/*.sh packaging/snap/prepare.sh` (também no job
`packaging` de `quality.yml`).

Publicação: `Release Candidate` (entrada `rc`) cria a tag
`<versão>-rc.<n>` como pre-release e anexa todos os pacotes; publicar uma
release estável dispara `Release Deploy`, que anexa os pacotes e publica o
snap no canal `stable` (segredo `SNAPCRAFT_STORE_CREDENTIALS`). Ao alterar a
versão, atualize também a entrada `<release>` do metainfo.
