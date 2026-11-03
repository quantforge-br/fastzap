#!/usr/bin/env bash
# Mark an official build: FASTZAP_BUILD_* are read at compile time by
# src-tauri/src/build_info.rs. Exported to later GitHub Actions steps and
# written to build-info.env for builds in containers (snap).
set -euo pipefail

PACKAGING="${1:?usage: build-info.sh <packaging-name>}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

{
  echo "FASTZAP_BUILD_CHANNEL=Official"
  echo "FASTZAP_BUILD_PROVIDER=GitHub Actions"
  echo "FASTZAP_BUILD_REPOSITORY=${GITHUB_REPOSITORY:-unknown}"
  echo "FASTZAP_BUILD_PACKAGING=${PACKAGING}"
} > "${HERE}/build-info.env"

if [ -n "${GITHUB_ENV:-}" ]; then
  cat "${HERE}/build-info.env" >> "${GITHUB_ENV}"
fi
cat "${HERE}/build-info.env"
