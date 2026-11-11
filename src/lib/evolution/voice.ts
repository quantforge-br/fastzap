// Pure helpers of the voice message player.
import type { ChatMessage } from "$lib/ipc";

/** Playback rates the speed button cycles through, as in WhatsApp. */
export const SPEEDS = [1, 1.5, 2] as const;

/** Bars drawn for every audio, whatever the length of its waveform. */
export const BAR_COUNT = 40;

/** Lowest bar height, so silence still shows a dot. */
const MIN_HEIGHT = 0.12;

/** `m:ss`, or `h:mm:ss` from one hour; invalid input reads as zero. */
export function formatDuration(seconds: number): string {
  const total = Number.isFinite(seconds) && seconds > 0 ? Math.floor(seconds) : 0;
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

/**
 * Bar heights in (0, 1] from WhatsApp's waveform bytes (0–100), resampled to
 * `count` bars by averaging and scaled to the loudest bar. Audios without a
 * waveform get flat neutral bars.
 */
export function waveformBars(waveform: readonly number[], count = BAR_COUNT): number[] {
  if (count <= 0) return [];
  if (!waveform.length) return Array(count).fill(0.3);
  const bars: number[] = [];
  for (let i = 0; i < count; i++) {
    const start = Math.floor((i * waveform.length) / count);
    const end = Math.max(start + 1, Math.floor(((i + 1) * waveform.length) / count));
    const slice = waveform.slice(start, end);
    bars.push(slice.reduce((sum, v) => sum + v, 0) / slice.length);
  }
  const peak = Math.max(...bars);
  return bars.map((v) => (peak > 0 ? Math.max(MIN_HEIGHT, v / peak) : MIN_HEIGHT));
}

/** The rate after `rate` in `SPEEDS`, wrapping to 1x. */
export function nextSpeed(rate: number): number {
  const i = SPEEDS.findIndex((s) => s === rate);
  return SPEEDS[(i + 1) % SPEEDS.length];
}

/** Speed button text: `1×`, `1,5×` (with the locale's decimal sign), `2×`. */
export function speedLabel(rate: number, locale: string): string {
  return `${rate.toLocaleString(locale)}×`;
}

/** Fraction played in [0, 1]. */
export function progress(current: number, duration: number): number {
  if (!(duration > 0) || !Number.isFinite(current)) return 0;
  return Math.min(1, Math.max(0, current / duration));
}

/** The audio right after `id` in the conversation, which plays next; else null. */
export function nextAudio(messages: readonly ChatMessage[], id: string): ChatMessage | null {
  const i = messages.findIndex((m) => m.id === id);
  const next = i < 0 ? undefined : messages[i + 1];
  return next && next.kind === "audio" && !next.revoked ? next : null;
}
