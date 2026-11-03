// Display helpers for download rows.

/** Middle-truncate a file name, keeping its extension (ZapZap: 38 chars). */
export function elideName(name: string, max = 38): string {
  const chars = [...name];
  if (chars.length <= max) return name;
  const dot = name.lastIndexOf(".");
  const ext = dot > 0 && name.length - dot <= 10 ? [...name.slice(dot)] : [];
  const keep = max - ext.length - 1;
  const head = Math.ceil(keep / 2);
  const stem = chars.slice(0, chars.length - ext.length);
  return `${stem.slice(0, head).join("")}…${stem.slice(stem.length - (keep - head)).join("")}${ext.join("")}`;
}
