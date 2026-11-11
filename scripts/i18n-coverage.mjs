// Report msgids used by the source that are missing from a translated catalog.
// Extraction mirrors the call sites: t()/tn() in the frontend and tr() in Rust.
// Run with \`pnpm i18n:coverage [po/code.po]\`.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import { parsePo, toCatalog } from "./po2json.mjs";

const root = new URL("..", import.meta.url).pathname;
const poPath = process.argv[2] ?? join(root, "po/pt_BR.po");

const SKIP_DIRS = new Set(["generated", "node_modules", ".svelte-kit", "build"]);
const SKIP_FILE = /(\.test\.ts|\.d\.ts|i18n\/index\.svelte\.ts)$/;

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (SKIP_DIRS.has(name)) continue;
    if (statSync(path).isDirectory()) walk(path, out);
    else if (/\.(svelte|ts|rs)$/.test(name) && !SKIP_FILE.test(relative(root, path))) out.push(path);
  }
  return out;
}

// Decode a JS/Rust double-quoted literal body into the runtime string.
function unescape(body) {
  return body.replace(/\\(u\{[0-9a-fA-F]+\}|u[0-9a-fA-F]{4}|x[0-9a-fA-F]{2}|n|t|r|"|\\)/g, (_, esc) => {
    if (esc.startsWith("u{")) return String.fromCodePoint(parseInt(esc.slice(2, -1), 16));
    if (esc.startsWith("u")) return String.fromCharCode(parseInt(esc.slice(1), 16));
    if (esc.startsWith("x")) return String.fromCharCode(parseInt(esc.slice(1), 16));
    return { n: "\n", t: "\t", r: "\r", '"': '"', "\\": "\\" }[esc] ?? esc;
  });
}

const CALL = /(?<![\w.])(tn|t|tr)\(\s*"((?:[^"\\]|\\.)*)"/g;

function extract(path) {
  const text = readFileSync(path, "utf8");
  const found = [];
  for (const match of text.matchAll(CALL)) {
    const [, callee, body] = match;
    const isRust = path.endsWith(".rs");
    if (isRust !== (callee === "tr")) continue;
    // Skip i18n infra and non-user-facing catalog helpers.
    found.push({ id: unescape(body), where: relative(root, path) });
  }
  return found;
}

const files = [...walk(join(root, "src")), ...walk(join(root, "src-tauri/src"))];
const used = new Map();
for (const file of files) for (const hit of extract(file)) if (hit.id) used.set(hit.id, hit.where);

const catalog = toCatalog(parsePo(readFileSync(poPath, "utf8")));
const messages = catalog.messages;
const plurals = catalog.plurals;

const missing = [...used].filter(([id]) => !(id in messages) && !(id in plurals));
const untranslated = missing.filter(([id]) => id.trim());

console.log("catalog: " + relative(root, poPath));
console.log("source strings: " + used.size);
console.log("translated: " + (used.size - untranslated.length));
console.log("missing: " + untranslated.length);
for (const [id, where] of untranslated.sort((a, b) => a[1].localeCompare(b[1]))) {
  console.log("  " + where + ": " + JSON.stringify(id));
}
process.exitCode = untranslated.length ? 1 : 0;
