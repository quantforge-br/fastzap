// Report msgids used by the source that are missing from a translated catalog.
// Extraction mirrors the call sites: t()/tn() in the frontend and tr() in Rust,
// plus literals rendered by Svelte markup and literals held in lookup tables.
// Run with: pnpm i18n:coverage [po/code.po].
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

const CALL = /(?<![\w.])(tr_args|tn|t|tr)\(\s*"((?:[^"\\]|\\.)*)"/g;
// t(NAME) or t(NAME[...]) reads a literal declared elsewhere in the file.
const INDIRECT = /(?<![\w.])(?:tn|t)\(\s*([A-Za-z_$][\w$]*)\s*(?:\[|[),])/g;
const ERROR = /#\[error\("((?:[^"\\]|\\.)*)"\)/g;
const TRANSLATABLE_ATTR = /\b(?:title|aria-label|aria-description|placeholder|alt|label)="([^"]*)"/g;

// Literal strings that a named constant can hold (object values, ternaries).
function literalsOf(text, name) {
  const start = text.search(new RegExp("\\bconst\\s+" + name + "\\b"));
  if (start < 0) return [];
  const eq = text.indexOf("=", start);
  if (eq < 0) return [];
  const rest = text.slice(eq + 1);
  const end = rest.search(/\n\s*(?:const|let|function|export|<\/script>)/);
  const body = end < 0 ? rest : rest.slice(0, end);
  return [...body.matchAll(/"((?:[^"\\]|\\.)*)"/g)]
    .map((m) => unescape(m[1]))
    .filter((id) => /[A-Z]/.test(id));
}

// Remove Svelte expressions, respecting quotes and nested braces.
function stripExpressions(text) {
  let out = "";
  let i = 0;
  while (i < text.length) {
    if (text[i] !== "{") {
      out += text[i];
      i += 1;
      continue;
    }
    let depth = 0;
    let quote = null;
    while (i < text.length) {
      const c = text[i];
      if (quote) {
        if (c === "\\") i += 1;
        else if (c === quote) quote = null;
      } else if (c === '"' || c === "'" || c === String.fromCharCode(96)) quote = c;
      else if (c === "{") depth += 1;
      else if (c === "}") {
        depth -= 1;
        if (depth === 0) {
          i += 1;
          break;
        }
      }
      i += 1;
    }
  }
  return out;
}

// Text nodes and translatable attributes of the markup outside <script>/<style>.
function markupStrings(text) {
  const markup = stripExpressions(
    text
      .replace(/<script\b[\s\S]*?<\/script>/gi, "")
      .replace(/<style\b[\s\S]*?<\/style>/gi, ""),
  );
  const found = [];
  for (const m of markup.matchAll(TRANSLATABLE_ATTR)) found.push(m[1]);
  for (const m of markup.matchAll(/>([^<>]+)</g)) found.push(m[1]);
  return found
    .map((s) => s.replace(/&[a-z]+;|&#\d+;/gi, " ").replace(/\s+/g, " ").trim())
    .filter((s) => /[A-Za-z]/.test(s) && !/^[{}().,;:!?|&/+*=-]+$/.test(s));
}

function extract(path) {
  const text = readFileSync(path, "utf8");
  const isRust = path.endsWith(".rs");
  const found = [];
  for (const match of text.matchAll(CALL)) {
    const [, callee, body] = match;
    if (isRust !== (callee === "tr" || callee === "tr_args")) continue;
    found.push({ id: unescape(body), where: relative(root, path) });
  }
  if (isRust) {
    // Static error messages reach the UI through CommandError; strings built
    // with format placeholders carry runtime values and cannot be matched.
    for (const m of text.matchAll(ERROR)) {
      const id = unescape(m[1]);
      if (!/[{}]/.test(id)) found.push({ id, where: relative(root, path) });
    }
  }
  if (!isRust) {
    for (const match of text.matchAll(INDIRECT)) {
      for (const id of literalsOf(text, match[1])) found.push({ id, where: relative(root, path) });
    }
    if (path.endsWith(".svelte")) {
      for (const id of markupStrings(text)) found.push({ id, where: relative(root, path) });
    }
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
