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

/** Delivery mark of our own messages. */
export function statusMark(status: Status): string {
  return { pending: "🕓", unknown: "⚠", failed: "✕", sent: "✓", delivered: "✓✓", read: "✓✓" }[status];
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
