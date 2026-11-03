#!/usr/bin/env node
// Regenerate src-tauri/resources/hunspell-catalog.json from a pinned commit
// of wooorm/dictionaries: every `dictionaries/<code>/index.{aff,dic}` with
// its git blob SHA-1 and size, which the app verifies after downloading.
// Usage: node scripts/hunspell-catalog.mjs <commit-sha>
import { writeFileSync } from "node:fs";

const commit = process.argv[2];
if (!/^[0-9a-f]{40}$/.test(commit ?? "")) {
  console.error("usage: node scripts/hunspell-catalog.mjs <40-hex commit>");
  process.exit(1);
}
const response = await fetch(`https://api.github.com/repos/wooorm/dictionaries/git/trees/${commit}?recursive=1`, {
  headers: { Accept: "application/vnd.github+json", "User-Agent": "fastzap-catalog" },
});
if (!response.ok) throw new Error(`GitHub returned ${response.status}`);
const tree = await response.json();
if (tree.truncated) throw new Error("tree is truncated");

const dictionaries = {};
for (const entry of tree.tree) {
  const match = /^dictionaries\/([A-Za-z0-9-]+)\/index\.(aff|dic)$/.exec(entry.path);
  if (!match || entry.type !== "blob") continue;
  const [, code, ext] = match;
  dictionaries[code] ??= {};
  dictionaries[code][ext] = { sha: entry.sha, size: entry.size };
}
const entries = Object.entries(dictionaries)
  .filter(([, files]) => files.aff && files.dic)
  .sort(([a], [b]) => a.localeCompare(b))
  .map(([code, files]) => ({ code, aff: files.aff, dic: files.dic }));

const path = new URL("../src-tauri/resources/hunspell-catalog.json", import.meta.url);
writeFileSync(path, `${JSON.stringify({ repository: "wooorm/dictionaries", commit, dictionaries: entries }, null, 1)}\n`);
console.log(`${entries.length} dictionaries at ${commit}`);
