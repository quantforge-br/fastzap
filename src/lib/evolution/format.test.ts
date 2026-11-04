import { describe, expect, it } from "vitest";

import type { ChatMessage } from "$lib/ipc";

import {
  dayLabel,
  jidForNumber,
  linkParts,
  listTime,
  mergeMessages,
  previewText,
  senderColor,
  startsDay,
  startsGroup,
  statusIcon,
} from "./format";

const t = (s: string) => s;

function message(id: string, ts: number, seq: number): ChatMessage {
  return {
    seq,
    id,
    chat: "1@s.whatsapp.net",
    sender: "",
    sender_name: "",
    from_me: false,
    ts,
    kind: "text",
    body: id,
    file_name: "",
    mime: "",
    quote: null,
    status: "sent",
    edited: false,
    revoked: false,
    reactions: [],
  };
}

describe("evolution chat formatting", () => {
  it("previews media with labels", () => {
    expect(previewText("text", "hi", t)).toBe("hi");
    expect(previewText("image", "", t)).toBe("Photo");
    expect(previewText("document", "cv.pdf", t)).toBe("Document: cv.pdf");
    expect(statusIcon("read")).toBe("checks");
    expect(statusIcon("failed")).toBe("alert");
  });

  it("formats times relative to today", () => {
    const now = new Date(2026, 0, 2, 12, 0);
    const today = Math.floor(new Date(2026, 0, 2, 9, 5).getTime() / 1000);
    const before = Math.floor(new Date(2025, 11, 30, 9, 5).getTime() / 1000);
    expect(listTime(today, "en-GB", now)).toBe("09:05");
    expect(listTime(before, "en-GB", now)).toBe("30/12/25");
    expect(listTime(0, "en-GB", now)).toBe("");
  });

  it("builds JIDs from numbers", () => {
    expect(jidForNumber("+55 (11) 99999-9999")).toBe("5511999999999@s.whatsapp.net");
    expect(jidForNumber("123")).toBeNull();
  });

  it("merges pages without duplicates and splits days", () => {
    const merged = mergeMessages([message("b", 20, 2)], [message("a", 10, 1), message("b", 20, 2)]);
    expect(merged.map((m) => m.id)).toEqual(["a", "b"]);
    expect(startsDay(undefined, merged[0])).toBe(true);
    expect(startsDay(merged[0], merged[1])).toBe(false);
  });

  it("labels days relative to today", () => {
    const now = new Date(2026, 10, 4, 12, 0);
    const at = (d: number) => Math.floor(new Date(2026, 10, d, 9, 0).getTime() / 1000);
    expect(dayLabel(at(4), "en-GB", t, now)).toBe("Today");
    expect(dayLabel(at(3), "en-GB", t, now)).toBe("Yesterday");
    expect(dayLabel(at(1), "en-GB", t, now)).toBe("Sunday");
    expect(dayLabel(at(28 - 31), "en-GB", t, now)).toBe("28/10/2026");
  });

  it("groups bubbles by sender", () => {
    const a = { ...message("a", 10, 1), sender: "x" };
    const b = { ...message("b", 11, 2), sender: "x" };
    const c = { ...message("c", 12, 3), sender: "y" };
    const d = { ...message("d", 13, 4), from_me: true };
    expect(startsGroup(undefined, a)).toBe(true);
    expect(startsGroup(a, b)).toBe(false);
    expect(startsGroup(b, c)).toBe(true);
    expect(startsGroup(c, d)).toBe(true);
  });

  it("finds links without trailing punctuation", () => {
    expect(linkParts("see https://a.b/c?d=1, ok")).toEqual([
      { text: "see " },
      { text: "https://a.b/c?d=1", url: "https://a.b/c?d=1" },
      { text: ", ok" },
    ]);
    expect(linkParts("plain")).toEqual([{ text: "plain" }]);
    expect(senderColor("1@s.whatsapp.net")).toBe(senderColor("1@s.whatsapp.net"));
  });
});
