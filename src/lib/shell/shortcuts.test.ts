import { describe, expect, it } from "vitest";

import { shortcutFor } from "./shortcuts";

const key = (k: string, mods: Partial<Record<"ctrlKey" | "metaKey" | "altKey" | "shiftKey", boolean>> = {}) => ({
  key: k,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

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
});
