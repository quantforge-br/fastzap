#!/usr/bin/env bash
# Record build provenance: FASTZAP_BUILD_* are read at compile time by
# src-tauri/src/build_info.rs. Exported to later GitHub Actions steps and
# written to build-info.env for builds in containers (snap).
#
# Only GitHub Actions runs of the official repository are marked Official;
# src-tauri/src/update.rs matches these values and the packaging names
# (DEB, macOS, Windows x86_64, Windows arm64) exactly.
set -euo pipefail

PACKAGING="${1:?usage: build-info.sh <DEB|RPM|AppImage|Flatpak|Snap|macOS|Windows x86_64|Windows arm64>}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OFFICIAL_REPOSITORY="quantmind-br/fastzap"
REPOSITORY="${GITHUB_REPOSITORY:-unknown}"

{
  if [ "${GITHUB_ACTIONS:-}" = "true" ] && [ "${REPOSITORY}" = "${OFFICIAL_REPOSITORY}" ]; then
    echo "FASTZAP_BUILD_CHANNEL=Official"
    echo "FASTZAP_BUILD_PROVIDER=GitHub Actions"
  else
    echo "FASTZAP_BUILD_CHANNEL=Unknown"
    echo "FASTZAP_BUILD_PROVIDER="
  fi
  echo "FASTZAP_BUILD_REPOSITORY=${REPOSITORY}"
  echo "FASTZAP_BUILD_PACKAGING=${PACKAGING}"
} > "${HERE}/build-info.env"

if [ -n "${GITHUB_ENV:-}" ]; then
  cat "${HERE}/build-info.env" >> "${GITHUB_ENV}"
fi
cat "${HERE}/build-info.env"
