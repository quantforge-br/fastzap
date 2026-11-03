#!/usr/bin/env bash
# Print the app version; with --check, fail unless package.json, Cargo.toml
# and tauri.conf.json agree.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

node - "${ROOT}" "${1:-}" <<'JS'
const fs = require("node:fs");
const path = require("node:path");

const [root, mode] = process.argv.slice(2);
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const cargo = read("src-tauri/Cargo.toml").match(/^\[package\][^[]*?^version\s*=\s*"([^"]+)"/m);
const versions = {
  "src-tauri/tauri.conf.json": JSON.parse(read("src-tauri/tauri.conf.json")).version,
  "package.json": JSON.parse(read("package.json")).version,
  "src-tauri/Cargo.toml": cargo ? cargo[1] : null,
};
if (mode === "--check" && new Set(Object.values(versions)).size !== 1) {
  for (const [file, version] of Object.entries(versions)) console.error(`${file}: ${version}`);
  console.error("versions differ");
  process.exit(1);
}
console.log(versions["src-tauri/tauri.conf.json"]);
JS
