// Pure helpers of the Evolution Go chat UI.
import type { ChatMessage, Connection } from "$lib/ipc";
import type { MessageKind } from "$lib/ipc/generated/MessageKind";
import type { Status } from "$lib/ipc/generated/Status";

type Translate = (source: string, params?: Record<string, string | number>) => string;

/** Label for media and other non-text messages. */
export function kindLabel(kind: MessageKind, t: Translate): string {
  return {
    text: "",
    image: t("Photo"),
    video: t("Video"),
    audio: t("Audio"),
    document: t("Document"),
    sticker: t("Sticker"),
    location: t("Location"),
    contact: t("Contact"),
    unsupported: t("Unsupported message"),
  }[kind];
}

/** One-line preview of a message for chat lists and replies. */
export function previewText(kind: MessageKind, body: string, t: Translate): string {
  const label = kindLabel(kind, t);
  if (!label) return body;
  return body ? `${label}: ${body}` : label;
}

/** Icon of the delivery state of our own messages. */
export function statusIcon(status: Status): "clock" | "check" | "checks" | "alert" {
  return ({ pending: "clock", unknown: "alert", failed: "alert", sent: "check", delivered: "checks", read: "checks" } as const)[status];
}

function sameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

/** Time for today's messages, the date otherwise. */
export function listTime(ts: number, locale: string, now = new Date()): string {
  if (!ts) return "";
  const date = new Date(ts * 1000);
  if (sameDay(date, now)) return date.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
  return date.toLocaleDateString(locale, { day: "2-digit", month: "2-digit", year: "2-digit" });
}

export function bubbleTime(ts: number, locale: string): string {
  return ts ? new Date(ts * 1000).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" }) : "";
}

/** Day separators: true when `message` starts a new day after `previous`. */
export function startsDay(previous: ChatMessage | undefined, message: ChatMessage): boolean {
  if (!previous) return true;
  return !sameDay(new Date(previous.ts * 1000), new Date(message.ts * 1000));
}

/** True when `message` opens a run of bubbles from one sender (it gets the tail). */
export function startsGroup(previous: ChatMessage | undefined, message: ChatMessage): boolean {
  return (
    !previous ||
    startsDay(previous, message) ||
    previous.from_me !== message.from_me ||
    (!message.from_me && previous.sender !== message.sender)
  );
}

/** Day separator text: today, yesterday, the weekday within a week, else the date. */
export function dayLabel(ts: number, locale: string, t: Translate, now = new Date()): string {
  const date = new Date(ts * 1000);
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const day = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const days = Math.round((today.getTime() - day.getTime()) / 86_400_000);
  if (days === 0) return t("Today");
  if (days === 1) return t("Yesterday");
  if (days > 1 && days < 7) return date.toLocaleDateString(locale, { weekday: "long" });
  return date.toLocaleDateString(locale, { day: "2-digit", month: "2-digit", year: "numeric" });
}

/** Text split into plain parts and web links. */
export function linkParts(text: string): { text: string; url?: string }[] {
  const parts: { text: string; url?: string }[] = [];
  let last = 0;
  for (const match of text.matchAll(/\bhttps?:\/\/[^\s<>"]+[^\s<>".,;:!?)\]'}]/gi)) {
    const start = match.index ?? 0;
    if (start > last) parts.push({ text: text.slice(last, start) });
    parts.push({ text: match[0], url: match[0] });
    last = start + match[0].length;
  }
  if (last < text.length) parts.push({ text: text.slice(last) });
  return parts;
}

// Sender name colors in groups, as WhatsApp gives each member one.
const SENDER_COLORS = ["#06cf9c", "#d88deb", "#fc9775", "#53bdeb", "#ffd279", "#a5b337", "#ff72a1", "#7f66ff"];

export function senderColor(jid: string): string {
  let hash = 0;
  for (const char of jid) hash = (hash * 31 + char.charCodeAt(0)) | 0;
  return SENDER_COLORS[Math.abs(hash) % SENDER_COLORS.length];
}

/** A JID for a phone number typed with country code, or null. */
export function jidForNumber(input: string): string | null {
  const digits = input.replace(/\D/g, "");
  if (digits.length < 7 || digits.length > 15) return null;
  return `${digits}@s.whatsapp.net`;
}

/** Merge a newer page into the loaded list, keeping order by seq position. */
export function mergeMessages(current: ChatMessage[], incoming: ChatMessage[]): ChatMessage[] {
  const byId = new Map(current.map((m) => [m.id, m]));
  for (const m of incoming) byId.set(m.id, m);
  return [...byId.values()].sort((a, b) => a.ts - b.ts || a.seq - b.seq);
}

export function connectionText(c: Connection, t: Translate): string {
  switch (c.state) {
    case "online":
      return t("Connected");
    case "connecting":
      return t("Connecting…");
    case "pairing":
      return t("Waiting for WhatsApp to be linked");
    case "logged_out":
      return t("WhatsApp was unlinked from this instance");
    case "banned":
      return t("WhatsApp temporarily banned this number");
    case "offline":
      return c.reason || t("Offline");
    case "stopped":
      return c.reason || t("Not running");
  }
}
