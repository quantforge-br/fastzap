// Pure helpers for the account sidebar.

/** Up to two initials from an account name, used when it has no avatar. */
export function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return "?";
  const letters = words.length === 1 ? [...words[0]].slice(0, 2) : [[...words[0]][0], [...words[1]][0]];
  return letters.join("").toUpperCase();
}

/** Badge text for an unread count; empty when there is nothing unread. */
export function unreadLabel(count: number): string {
  if (!count || count < 1) return "";
  return count > 99 ? "99+" : String(count);
}

/** Whether an account icon can be used as an <img> source. */
export function isImageIcon(icon: string | null): icon is string {
  return !!icon && icon.startsWith("data:image/");
}
