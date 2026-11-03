import { describe, expect, it } from "vitest";

import { compilePlural } from "./plural";
import { catalogCode, setLocale, t, tn } from "./index.svelte";

describe("plural expressions", () => {
  it("evaluates gettext rules", () => {
    const pt = compilePlural("(n > 1)");
    expect([0, 1, 2].map(pt)).toEqual([0, 0, 1]);
    const ar = compilePlural(
      "(n == 0 ? 0 : n == 1 ? 1 : n == 2 ? 2 : n % 100 >= 3 && n % 100 <= 10 ? 3 : n % 100 >= 11 && n % 100 <= 99 ? 4 : 5)",
    );
    expect([0, 1, 2, 5, 11, 100].map(ar)).toEqual([0, 1, 2, 3, 4, 5]);
    const ru = compilePlural(
      "(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)",
    );
    expect([1, 3, 5, 11, 21].map(ru)).toEqual([0, 1, 2, 2, 0]);
  });

  it("falls back on invalid input", () => {
    expect(compilePlural("n ? ")(1)).toBe(0);
  });
});

describe("catalogs", () => {
  it("resolves locales like the backend", () => {
    expect(catalogCode("pt_BR")).toBe("pt_BR");
    expect(catalogCode("de_AT")).toBe("de");
    expect(catalogCode("ru")).toBe("ru_RU");
    expect(catalogCode("xx")).toBeNull();
  });

  it("translates and interpolates", () => {
    setLocale("pt_BR");
    expect(t("Quit")).toBe("Sair");
    expect(t("Unknown {x}", { x: 1 })).toBe("Unknown 1");
    expect(tn("{count} available", "{count} available", 1)).toMatch(/1/);
    setLocale("en");
    expect(t("Quit")).toBe("Quit");
  });
});
