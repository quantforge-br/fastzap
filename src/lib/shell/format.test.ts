import { describe, expect, it } from "vitest";

import { unreadLabel } from "./format";

describe("unreadLabel", () => {
  it("caps large counts", () => {
    expect(unreadLabel(0)).toBe("");
    expect(unreadLabel(7)).toBe("7");
    expect(unreadLabel(150)).toBe("99+");
  });
});
