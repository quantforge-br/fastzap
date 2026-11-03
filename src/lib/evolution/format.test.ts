import { describe, expect, it } from "vitest";

import type { ChatMessage } from "$lib/ipc";

import { jidForNumber, listTime, mergeMessages, previewText, startsDay, statusMark } from "./format";

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
    expect(statusMark("read")).toBe("✓✓");
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
});
