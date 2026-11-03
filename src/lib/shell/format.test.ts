import { describe, expect, it } from "vitest";

import { initials, isImageIcon, unreadLabel } from "./format";

describe("initials", () => {
  it("uses the first letters of the first two words", () => {
    expect(initials("work phone")).toBe("WP");
    expect(initials("Ana")).toBe("AN");
    expect(initials("  ")).toBe("?");
  });
});

describe("unreadLabel", () => {
  it("caps large counts", () => {
    expect(unreadLabel(0)).toBe("");
    expect(unreadLabel(7)).toBe("7");
    expect(unreadLabel(150)).toBe("99+");
  });
});

describe("isImageIcon", () => {
  it("only accepts image data URLs", () => {
    expect(isImageIcon("data:image/png;base64,AA")).toBe(true);
    expect(isImageIcon("<svg/>")).toBe(false);
    expect(isImageIcon(null)).toBe(false);
  });
});
