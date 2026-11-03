#!/usr/bin/env bash
# Put the snap recipe where snapcraft looks for it (snap/snapcraft.yaml).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "${ROOT}/snap"
cp "${ROOT}/packaging/snap/snapcraft.yaml" "${ROOT}/snap/snapcraft.yaml"
