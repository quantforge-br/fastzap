// Convert po/*.po (imported from ZapZap) into the JSON catalogs embedded by
// src-tauri/build.rs and loaded by src/lib/i18n. Run with `pnpm i18n`.
//
// Output: { plural: "<C expression>", messages: {id: str}, plurals: {id: [forms]} }.
// "ZapZap" is renamed to "FastZap" in ids and translations; fuzzy and
// untranslated entries are dropped.
import { readFileSync, readdirSync, writeFileSync, mkdirSync, rmSync } from "node:fs";
import { join, basename } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const poDir = join(root, "po");
const outDir = join(root, "src/lib/i18n/catalogs");

function unquote(line) {
  return JSON.parse(line.slice(line.indexOf('"')));
}

export function parsePo(text) {
  const entries = [];
  let entry = null;
  let field = null;
  const flush = () => {
    if (entry && entry.msgid !== undefined) entries.push(entry);
    entry = null;
    field = null;
  };
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (line === "") {
      flush();
      continue;
    }
    entry ??= { flags: [], msgstr: [] };
    if (line.startsWith("#,")) {
      entry.flags.push(...line.slice(2).split(",").map((f) => f.trim()));
    } else if (line.startsWith("#")) {
      continue;
    } else if (line.startsWith("msgctxt ")) {
      entry.msgctxt = unquote(line);
      field = "msgctxt";
    } else if (line.startsWith("msgid_plural ")) {
      entry.msgid_plural = unquote(line);
      field = "msgid_plural";
    } else if (line.startsWith("msgid ")) {
      entry.msgid = unquote(line);
      field = "msgid";
    } else if (line.startsWith("msgstr[")) {
      const index = Number(line.slice(7, line.indexOf("]")));
      entry.msgstr[index] = unquote(line);
      field = index;
    } else if (line.startsWith("msgstr ")) {
      entry.msgstr[0] = unquote(line);
      field = 0;
    } else if (line.startsWith('"')) {
      if (typeof field === "number") entry.msgstr[field] += unquote(line);
      else if (field) entry[field] += unquote(line);
    }
  }
  flush();
  return entries;
}

const rename = (s) => s.replaceAll("ZapZap", "FastZap");

export function toCatalog(entries) {
  const catalog = { plural: "n != 1", messages: {}, plurals: {} };
  for (const e of entries) {
    if (e.msgid === "") {
      const match = /plural=([^;]+);?/.exec(e.msgstr[0] ?? "");
      if (match) catalog.plural = match[1].trim();
      continue;
    }
    if (e.flags.includes("fuzzy") || e.msgctxt !== undefined) continue;
    if (e.msgid_plural !== undefined) {
      if (e.msgstr.some((s) => !s)) continue;
      catalog.plurals[rename(e.msgid)] = e.msgstr.map(rename);
    } else if (e.msgstr[0]) {
      catalog.messages[rename(e.msgid)] = rename(e.msgstr[0]);
    }
  }
  return catalog;
}

if (process.argv[1] && basename(process.argv[1]) === "po2json.mjs") {
  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
  for (const file of readdirSync(poDir).filter((f) => f.endsWith(".po")).sort()) {
    const catalog = toCatalog(parsePo(readFileSync(join(poDir, file), "utf8")));
    writeFileSync(join(outDir, file.replace(/\.po$/, ".json")), JSON.stringify(catalog, null, 1) + "\n");
  }
}
