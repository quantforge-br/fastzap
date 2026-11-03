import { describe, expect, it } from "vitest";

import { matches, searchKey, sizeLabel, summary } from "./format";

describe("spell checker format", () => {
  it("searches without accents or case", () => {
    expect(searchKey("Português")).toBe("portugues");
    expect(matches("portu", "Português", "pt")).toBe(true);
    expect(matches("PT_", "Portuguese", "pt_PT")).toBe(true);
    expect(matches("de", "Portuguese", "pt")).toBe(false);
    expect(matches("  ", "anything")).toBe(true);
  });

  it("summarizes the selection", () => {
    const more = (first: string, n: number) => `${first} and ${n} more`;
    expect(summary([], more)).toBe("");
    expect(summary(["English"], more)).toBe("English");
    expect(summary(["English", "German", "French"], more)).toBe("English and 2 more");
  });

  it("formats sizes", () => {
    expect(sizeLabel(2 * 1024 * 1024)).toBe("2.0 MB");
    expect(sizeLabel(300)).toBe("1 KB");
  });
});
