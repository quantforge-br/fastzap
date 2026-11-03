import { describe, expect, it } from "vitest";

import { defaultRegion, validateChat } from "./validate";

describe("open chat validation", () => {
  it("mirrors the backend rules", () => {
    expect(validateChat("", "11999999999", "")).toBe("missing_country");
    expect(validateChat("55", " ", "")).toBe("empty_number");
    expect(validateChat("55", "+55 11 99999", "")).toBe("duplicated_country_code");
    expect(validateChat("55", "0055 11", "")).toBe("duplicated_country_code");
    expect(validateChat("55", "123", "")).toBe("number_too_short");
    expect(validateChat("55", "1234567890123456", "")).toBe("number_too_long");
    expect(validateChat("55", "11 99999-9999", "x".repeat(501))).toBe("message_too_long");
    expect(validateChat("55", "(11) 99999-9999", "Hi")).toBeNull();
  });

  it("finds the locale region", () => {
    expect(defaultRegion(["pt-BR"])).toBe("BR");
    expect(defaultRegion(["de"])).toBe("DE");
    expect(defaultRegion(["??", "en-GB"])).toBe("GB");
  });
});
