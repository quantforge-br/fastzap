import { describe, expect, it } from "vitest";

import type { ChatMessage } from "$lib/ipc";

import { BAR_COUNT, formatDuration, nextAudio, nextSpeed, progress, speedLabel, waveformBars } from "./voice";

function message(id: string, kind: ChatMessage["kind"], revoked = false): ChatMessage {
  return {
    seq: 0,
    id,
    chat: "1@s.whatsapp.net",
    sender: "",
    sender_name: "",
    from_me: false,
    ts: 0,
    kind,
    body: "",
    file_name: "",
    mime: "",
    buttons: [],
    seconds: 0,
    waveform: [],
    voice: kind === "audio",
    quote: null,
    status: "sent",
    edited: false,
    revoked,
    reactions: [],
  };
}

describe("voice", () => {
  it("formats durations as WhatsApp does", () => {
    expect(formatDuration(0)).toBe("0:00");
    expect(formatDuration(17)).toBe("0:17");
    expect(formatDuration(41.9)).toBe("0:41");
    expect(formatDuration(600)).toBe("10:00");
    expect(formatDuration(3725)).toBe("1:02:05");
    expect(formatDuration(Number.NaN)).toBe("0:00");
    expect(formatDuration(-3)).toBe("0:00");
  });

  it("resamples the waveform to the bar count and scales it to the peak", () => {
    const bars = waveformBars([0, 50, 100, 50], 2);
    expect(bars).toEqual([0.25 / 0.75, 1]);
    expect(waveformBars(Array(64).fill(10))).toHaveLength(BAR_COUNT);
    expect(waveformBars(Array(64).fill(10)).every((b) => b === 1)).toBe(true);
    // Fewer bytes than bars repeat the bytes.
    expect(waveformBars([100, 0], 4)).toEqual([1, 1, 0.12, 0.12]);
  });

  it("draws neutral bars without a waveform and a dot for silence", () => {
    expect(waveformBars([], 3)).toEqual([0.3, 0.3, 0.3]);
    expect(waveformBars([0, 0], 2)).toEqual([0.12, 0.12]);
    expect(waveformBars([1, 2], 0)).toEqual([]);
  });

  it("cycles the speed through 1x, 1.5x and 2x", () => {
    expect(nextSpeed(1)).toBe(1.5);
    expect(nextSpeed(1.5)).toBe(2);
    expect(nextSpeed(2)).toBe(1);
    expect(nextSpeed(3)).toBe(1);
    expect(speedLabel(1.5, "pt-BR")).toBe("1,5×");
    expect(speedLabel(2, "en")).toBe("2×");
  });

  it("clamps the progress", () => {
    expect(progress(5, 10)).toBe(0.5);
    expect(progress(12, 10)).toBe(1);
    expect(progress(5, 0)).toBe(0);
    expect(progress(5, Number.POSITIVE_INFINITY)).toBe(0);
    expect(progress(Number.NaN, 10)).toBe(0);
  });

  it("plays next only the audio right after the finished one", () => {
    const list = [message("a", "audio"), message("b", "audio"), message("c", "text"), message("d", "audio")];
    expect(nextAudio(list, "a")?.id).toBe("b");
    expect(nextAudio(list, "b")).toBeNull();
    expect(nextAudio(list, "d")).toBeNull();
    expect(nextAudio(list, "x")).toBeNull();
    expect(nextAudio([message("a", "audio"), message("b", "audio", true)], "a")).toBeNull();
  });
});
