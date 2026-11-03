// Pure helpers for the account sidebar.

/** Badge text for an unread count; empty when there is nothing unread. */
export function unreadLabel(count: number): string {
  if (!count || count < 1) return "";
  return count > 99 ? "99+" : String(count);
}
