import { describe, expect, it } from "vitest";

import { DEFAULT_ICON, iconSrc, newIconSvg, parseIcon, PROFILE_PREFIX, serializeIcon } from "./icon";

const PNG = "data:image/png;base64,AAAA";

describe("account icons", () => {
  it("replaces both gradient colors", () => {
    const svg = newIconSvg(() => 0);
    expect(svg).not.toContain("#209232");
    expect(svg).not.toContain("#34c640");
    expect(svg).toContain("rgb(0, 0, 0)");
  });

  it("parses every stored form", () => {
    expect(parseIcon(null)).toEqual({ icon: DEFAULT_ICON, photo: null, usePhoto: false });
    expect(parseIcon("<svg/>")).toEqual({ icon: "<svg/>", photo: null, usePhoto: false });
    expect(parseIcon(PNG)).toEqual({ icon: DEFAULT_ICON, photo: PNG, usePhoto: true });
    expect(parseIcon(`${PROFILE_PREFIX}{bad`).icon).toBe(DEFAULT_ICON);
  });

  it("round-trips a profile", () => {
    const choice = { icon: "<svg/>", photo: PNG, usePhoto: false };
    const stored = serializeIcon(choice);
    expect(stored.startsWith(PROFILE_PREFIX)).toBe(true);
    expect(parseIcon(stored)).toEqual(choice);
    expect(iconSrc(stored)).toBe("data:image/svg+xml;charset=utf-8,%3Csvg%2F%3E");
    expect(iconSrc(serializeIcon({ ...choice, usePhoto: true }))).toBe(PNG);
    expect(serializeIcon({ icon: "<svg/>", photo: null, usePhoto: true })).toBe("<svg/>");
  });
});
