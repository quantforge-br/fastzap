#!/usr/bin/env bash
# Build as the user; only the package transaction runs with elevated privileges.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

if (( EUID == 0 )); then
  printf '%s\n' 'Run make install as your regular user, not with sudo.' >&2
  exit 1
fi
if [[ $(uname -s) != Linux ]]; then
  printf '%s\n' 'make install currently supports Arch Linux with pacman.' >&2
  exit 1
fi
for command in pnpm node cargo git makepkg pacman bsdtar sudo flock; do
  if ! command -v "$command" >/dev/null; then
    printf 'Missing required command: %s\n' "$command" >&2
    exit 1
  fi
done

# Pin the native target directory so user Cargo settings cannot redirect the DEB.
export CARGO_TARGET_DIR="$ROOT/src-tauri/target"
unset CARGO_BUILD_TARGET
mkdir -p "$CARGO_TARGET_DIR"
exec 9>"$CARGO_TARGET_DIR/fastzap-install.lock"
if ! flock -n 9; then
  printf '%s\n' 'Another make install is already running for this checkout.' >&2
  exit 1
fi

version="$(bash packaging/common/version.sh --check)"
revision="$(git rev-parse --short=7 HEAD)"
# Local builds include uncommitted files; the timestamp distinguishes rebuilds.
FASTZAP_LOCAL_VERSION="${version}.local.$(date -u +%Y%m%d%H%M%S).g${revision}"
export FASTZAP_LOCAL_VERSION
export FASTZAP_BUILD_CHANNEL=Unknown
export FASTZAP_BUILD_PROVIDER='Local source'
export FASTZAP_BUILD_REPOSITORY='quantforge-br/fastzap'
export FASTZAP_BUILD_PACKAGING=Arch

printf 'Building %s from the current working tree (including uncommitted changes).\n' "$FASTZAP_LOCAL_VERSION"
pnpm install --frozen-lockfile
# Another checkout that shared this target directory may have compiled the
# build script, with its own command lists and catalog path. A newer
# build.rs makes Cargo rebuild and rerun it from this checkout.
touch src-tauri/build.rs
pnpm tauri build --bundles deb -- --frozen

# Select the exact native artifact, never an arbitrary older DEB in bundle/deb.
case "$(uname -m)" in
  x86_64) deb_arch=amd64 ;;
  aarch64) deb_arch=arm64 ;;
  *) printf '%s\n' 'Unsupported architecture for local installation.' >&2; exit 1 ;;
esac
deb="$CARGO_TARGET_DIR/release/bundle/deb/FastZap_${version}_${deb_arch}.deb"
if [[ ! -s $deb ]]; then
  printf 'Build did not produce the expected DEB: %s\n' "$deb" >&2
  exit 1
fi

work="$(mktemp -d "$CARGO_TARGET_DIR/fastzap-install.XXXXXX")"
trap 'rm -rf -- "$work"' EXIT
cp packaging/arch/PKGBUILD "$work/PKGBUILD"
cp LICENSE "$work/LICENSE"
cp "$deb" "$work/FastZap.deb"
(
  cd "$work"
  # Ignore user output paths: retain exactly this package, not another checkout's.
  export PKGDEST="$work" SRCDEST="$work" SRCPKGDEST="$work" BUILDDIR="$work"
  makepkg --force --nodeps --nosign
)
shopt -s nullglob
packages=("$work"/dsr-fastzap-git-*.pkg.tar.*)
if (( ${#packages[@]} != 1 )); then
  printf 'Expected one built package, found %s.\n' "${#packages[@]}" >&2
  exit 1
fi
artifact_dir="$CARGO_TARGET_DIR/local-install"
mkdir -p "$artifact_dir"
artifact="$artifact_dir/$(basename "${packages[0]}")"
cp "${packages[0]}" "$artifact"
sudo pacman -U --noconfirm "$artifact"
pacman -Q dsr-fastzap-git
printf 'Installed from: %s\n' "$artifact"
printf '%s\n' 'If FastZap is running, quit it from its menu and reopen it to use the new binary.'
