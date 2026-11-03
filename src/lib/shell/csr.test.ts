import { describe, expect, it } from "vitest";

import { resolveCsrButtonTheme } from "./csr";

describe("resolveCsrButtonTheme", () => {
  it("keeps a known configured theme", () => {
    expect(resolveCsrButtonTheme(" Windows ", "GNOME")).toBe("windows");
  });

  it("falls back to the desktop's theme", () => {
    expect(resolveCsrButtonTheme("", "KDE")).toBe("plasma");
    expect(resolveCsrButtonTheme("unknown", "ubuntu:GNOME")).toBe("adwaita");
    expect(resolveCsrButtonTheme("", "Hyprland")).toBe("default");
  });
});
