// Spell checker labels and search, shared by the picker and the manager.
import { languageName } from "$lib/i18n/index.svelte";

export const MAX_LANGUAGES = 10;

/** Display name of an Enchant tag (`pt_PT` → "European Portuguese"). */
export function dictionaryLabel(tag: string): string {
  return languageName(tag.replace(/_/g, "-"));
}

/** Accent- and case-insensitive search key. */
export function searchKey(text: string): string {
  return text.normalize("NFKD").replace(/\p{M}/gu, "").toLowerCase();
}

export function matches(query: string, ...fields: string[]): boolean {
  const q = searchKey(query.trim());
  return !q || fields.some((f) => searchKey(f).includes(q));
}

/** "Portuguese", or "Portuguese and 2 more" in the settings row. */
export function summary(labels: string[], more: (first: string, count: number) => string): string {
  if (labels.length <= 1) return labels[0] ?? "";
  return more(labels[0], labels.length - 1);
}

export function sizeLabel(bytes: number): string {
  const mib = bytes / (1024 * 1024);
  return mib >= 1 ? `${mib.toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`;
}
