#!/usr/bin/env bash
# Print the app version; with --check, fail unless package.json, Cargo.toml,
# tauri.conf.json and the newest AppStream release agree.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [ "$#" -gt 1 ] || { [ "$#" -eq 1 ] && [ "$1" != "--check" ]; }; then
  echo "usage: version.sh [--check]" >&2
  exit 2
fi

node - "${ROOT}" "${1:-}" <<'JS'
const fs = require("node:fs");
const path = require("node:path");

const [root, mode] = process.argv.slice(2);
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const metainfo = "packaging/share/metainfo/io.github.quantmind_br.fastzap.metainfo.xml";
// Only the [package] table: other tables also have `version` keys.
const cargoPackage = read("src-tauri/Cargo.toml")
  .split(/^\[/m)
  .find((table) => table.startsWith("package]"));
const versions = {
  "src-tauri/tauri.conf.json": JSON.parse(read("src-tauri/tauri.conf.json")).version,
  "package.json": JSON.parse(read("package.json")).version,
  "src-tauri/Cargo.toml": cargoPackage?.match(/^version\s*=\s*"([^"]+)"/m)?.[1],
};
if (mode === "--check") {
  // Releases are listed newest first.
  versions[metainfo] = read(metainfo).match(/<release\s[^>]*version="([^"]+)"/)?.[1];
}
const missing = Object.entries(versions).filter(([, version]) => typeof version !== "string" || !version);
for (const [file] of missing) console.error(`${file}: version not found`);
if (missing.length > 0) process.exit(1);
if (new Set(Object.values(versions)).size !== 1) {
  for (const [file, version] of Object.entries(versions)) console.error(`${file}: ${version}`);
  console.error("versions differ");
  process.exit(1);
}
console.log(versions["src-tauri/tauri.conf.json"]);
JS
