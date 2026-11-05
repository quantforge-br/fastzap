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

### Atualizar a instalação local no Arch Linux

Na raiz do checkout, como usuário normal:

```bash
make install
```

O comando instala as dependências JS pelo lockfile, recompila frontend e
backend com Tauri e converte o DEB resultante em um pacote Arch
`dsr-fastzap-git`, instalado por `sudo pacman -U`. Requer `make`, `pnpm`,
Node, Cargo, Git, `makepkg` (base-devel), `bsdtar`, `flock`, `sudo` e as
dependências nativas de compilação. Não execute `sudo make install`.

O build usa a árvore atual, inclusive alterações não commitadas; não faz
commit, push nem busca código no GitHub. A versão local contém a versão do
app, data UTC e revisão Git. Futuros pacotes do repositório podem ter versão
inferior à local; nesse caso, instale-os explicitamente com `pacman -U`.
O artefato fica em `src-tauri/target/local-install/`, e o binário permanece
gerenciado pelo pacman em `/usr/bin/fastzap`. O canal é local/não oficial.
O nome do pacote é o mesmo do dsr-aur: a transação substitui essa instalação,
inclusive `/usr/bin/fastzap`, ícones e lançador, sem instalar uma segunda cópia.
O instalador toca `src-tauri/build.rs` antes do build, então cada
`make install` recompila a crate `fastzap` e reexecuta o build script desta
checkout, mesmo que outra worktree tenha usado o mesmo `src-tauri/target`.

Contas, histórico, preferências e keyring não são alterados pelo instalador.
Ele não fecha nem abre o app: se estiver rodando, use Sair e abra novamente
após a instalação. O suporte deste comando é específico para Arch Linux;
as demais plataformas continuam usando seus empacotadores acima.

O Flatpak é uma instalação independente: se ele e o pacote nativo estiverem
instalados, o lançador mostrará dois itens FastZap. `make install` não atualiza,
oculta nem remove o Flatpak. O atalho nativo é `fastzap.desktop`; o Flatpak
exporta `io.github.quantmind_br.fastzap.desktop`.

### Validação e publicação

Validadores: `packaging/common/version.sh --check`,
`desktop-file-validate packaging/share/applications/*.desktop`,
`appstreamcli validate --no-net packaging/share/metainfo/*.xml` e
`shellcheck packaging/common/*.sh packaging/snap/prepare.sh` (também no job
`packaging` de `quality.yml`).

Publicação: `Release Candidate` (entrada `rc`, inteiro positivo, só a partir
de `main`) cria a tag `<versão>-rc.<n>` no commit atual como pre-release e
anexa todos os pacotes; repetir o mesmo `rc` só reenvia os pacotes se a tag
apontar para o mesmo commit. Os binários de um RC trazem a versão base
(`X.Y.Z`), não o sufixo `-rc.N`. Publicar uma release estável, ou promover
uma pre-release, dispara `Release Deploy`: ele confere que a tag (com `v`
opcional) é a versão do app, anexa os pacotes e publica o snap no canal
`stable` (segredo `SNAPCRAFT_STORE_CREDENTIALS`). A release fica pública
antes de os pacotes chegarem, então o aviso de atualização pode apontar para
uma página ainda sem instaladores durante o build. Ao alterar a versão,
atualize também a entrada `<release>` do metainfo: `version.sh --check`
recusa a divergência. Os instaladores Windows não são assinados.
