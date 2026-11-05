import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { shortcutFor, type KeyLike } from "./shortcuts";

const key = (k: string, mods: Partial<Record<"ctrlKey" | "metaKey" | "altKey" | "shiftKey", boolean>> = {}) => ({
  key: k,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

/** `shortcutFor` of the script injected into the account pages. */
function pageShortcutFor(): (event: KeyLike) => string | null {
  const source = readFileSync(new URL("../../../src-tauri/src/webview/scripts/bridge.js", import.meta.url), "utf8");
  const start = source.indexOf("const shortcutFor = (event) => {");
  const end = source.indexOf("\n  };\n", start);
  if (start < 0 || end < 0) throw new Error("shortcutFor not found in bridge.js");
  return new Function(`${source.slice(start, end + "\n  };".length)}\nreturn shortcutFor;`)();
}

const MODIFIERS = ["ctrlKey", "metaKey", "altKey", "shiftKey"] as const;
const KEYS = [
  ...Array.from({ length: 12 }, (_, i) => `F${i + 1}`),
  ..."abcdefghijklmnopqrstuvwxyz",
  ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ",
  ..."0123456789",
  ..."+=-/?",
  "Escape",
];

/** Every key with each of the 16 modifier combinations. */
function* keyEvents(): Generator<KeyLike> {
  for (const k of KEYS) {
    for (let mask = 0; mask < 1 << MODIFIERS.length; mask++) {
      yield key(k, Object.fromEntries(MODIFIERS.map((m, bit) => [m, (mask & (1 << bit)) !== 0])));
    }
  }
}

const label = (e: KeyLike) => [...MODIFIERS.filter((m) => e[m]).map((m) => m.replace("Key", "")), e.key].join("+");

describe("local shortcuts", () => {
  it("maps the app shortcuts", () => {
    expect(shortcutFor(key("m", { ctrlKey: true }))).toBe("open_chat_by_number");
    expect(shortcutFor(key("J", { ctrlKey: true }))).toBe("downloads");
    expect(shortcutFor(key("2", { ctrlKey: true, altKey: true }))).toBe("theme_light");
    expect(shortcutFor(key("3", { ctrlKey: true }))).toBe("account_3");
    expect(shortcutFor(key("F11"))).toBe("fullscreen");
  });

  it("leaves other keys to the focused control", () => {
    expect(shortcutFor(key("m"))).toBeNull();
    expect(shortcutFor(key("c", { ctrlKey: true }))).toBeNull();
    expect(shortcutFor(key("V", { ctrlKey: true, shiftKey: true }))).toBeNull();
  });

  it("agrees with the account page map", () => {
    const page = pageShortcutFor();
    for (const e of keyEvents()) {
      // Plain-text paste exists only inside the account pages.
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && !e.altKey && (e.key === "V" || e.key === "v")) {
        expect(page(e), label(e)).toBe("paste_plain_text");
        expect(shortcutFor(e), label(e)).toBeNull();
      } else {
        expect(page(e), label(e)).toBe(shortcutFor(e));
      }
    }
  });
});
