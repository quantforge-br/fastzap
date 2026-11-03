import { describe, expect, it } from "vitest";

import { elideName } from "./format";

describe("elideName", () => {
  it("keeps short names and the extension of long ones", () => {
    expect(elideName("a.pdf")).toBe("a.pdf");
    const out = elideName(`${"x".repeat(60)}.pdf`, 20);
    expect([...out].length).toBe(20);
    expect(out.endsWith(".pdf")).toBe(true);
    expect(out).toContain("…");
  });
});
