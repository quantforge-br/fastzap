# Maintenance

## Versions

The version lives in three places that must match: `package.json`,
`src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`. When opening the next
cycle after a release, derive it from the last stable tag (`X.Y.Z` becomes
`X.Y.(Z+1)`); minor or major bumps are decided by the maintainer.

## Changelog

Every change or addition — code, documentation, tests, dependencies,
packaging, and workflows — goes into the section marked `In development` in
`CHANGELOG.md`, grouped under `Added`, `Changed`, `Fixed`, or `Removed`.

## Persisted preferences

Preferences live in `settings.json` in the app's configuration directory
(`$XDG_CONFIG_HOME/io.github.quantmind_br.fastzap/` on Linux). The file has a
`version` field; format changes must add a step in
`settings::store::migrate` instead of changing the meaning of a field.

## Translations

When updating `po/*.po`, run `pnpm i18n` and commit the generated JSON in
`src/lib/i18n/catalogs/`. New UI strings use the English text as `msgid`.

## Packaging

The manifests live in `packaging/` and the workflows in `.github/workflows/`;
see [architecture](architecture.md#packaging). After cloning, run
`git submodule update --init` (shared Flathub modules).

Local builds:

```bash
pnpm tauri build --bundles deb,rpm,appimage
flatpak-builder --user --install --force-clean --state-dir=.flatpak-builder \
  --install-deps-from=flathub .flatpak-build \
  packaging/flatpak/io.github.quantmind_br.fastzap.yml
packaging/snap/prepare.sh && snapcraft pack
```

### Updating the local installation on Arch Linux

At the checkout root, as a regular user:

```bash
make install
```

The command installs the JS dependencies from the lockfile, rebuilds the frontend
and backend with Tauri, and converts the resulting DEB into an Arch package
`dsr-fastzap-git`, installed via `sudo pacman -U`. It requires `make`, `pnpm`,
Node, Cargo, Git, `makepkg` (base-devel), `bsdtar`, `flock`, `sudo`, and the
native build dependencies. Do not run `sudo make install`.

The build uses the current tree, including uncommitted changes; it does not
commit, push, or fetch code from GitHub. The local version contains the app
version, UTC date, and Git revision. Future repository packages may carry a
lower version than the local one; in that case, install them explicitly with
`pacman -U`. The artifact stays in `src-tauri/target/local-install/`, and the
binary remains managed by pacman at `/usr/bin/fastzap`. The channel is
local/unofficial. The package name matches dsr-aur's: the transaction replaces
that installation, including `/usr/bin/fastzap`, icons, and launcher, without
installing a second copy. The installer touches `src-tauri/build.rs` before the
build, so each `make install` recompiles the `fastzap` crate and re-runs this
checkout's build script, even if another worktree has used the same
`src-tauri/target`.

Accounts, history, preferences, and keyring are untouched by the installer.
It neither closes nor opens the app: if it is running, Quit it and reopen
it after the installation. Support for this command is Arch Linux-specific;
all other platforms keep using their packagers above.

Flatpak is an independent installation: if both it and the native package are
installed, the launcher will show two FastZap items. `make install` does not
update, hide, or remove the Flatpak. The native shortcut is `fastzap.desktop`;
the Flatpak exports `io.github.quantmind_br.fastzap.desktop`.

### Validation and publishing

Validators: `packaging/common/version.sh --check`,
`desktop-file-validate packaging/share/applications/*.desktop`,
`appstreamcli validate --no-net packaging/share/metainfo/*.xml`, and
`shellcheck packaging/common/*.sh packaging/snap/prepare.sh` (also in the
`packaging` job of `quality.yml`).

Publishing: `Release Candidate` (input `rc`, a positive integer, only from
`main`) tags `<version>-rc.<n>` on the current commit as a pre-release and
attaches all packages; repeating the same `rc` only re-uploads the packages if
the tag points at the same commit. Binaries from an RC carry the base version
(`X.Y.Z`), not the `-rc.N` suffix. Publishing a stable release, or promoting
a pre-release, triggers `Release Deploy`: it checks that the tag (with optional
`v`) matches the app version, attaches the packages, and publishes the snap to
the `stable` channel (secret `SNAPCRAFT_STORE_CREDENTIALS`). The release goes
public before the packages arrive, so the update notice may point at a page
with no installers yet during the build. When changing the version, also update
the metainfo `<release>` entry: `version.sh --check` rejects the mismatch.
Windows installers are unsigned.
