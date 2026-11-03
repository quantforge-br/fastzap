// App shortcuts while the local UI has focus; mirrors `shortcutFor` in
// src-tauri/src/webview/scripts/bridge.js, which covers the account pages.
export type KeyLike = Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">;

export function shortcutFor(event: KeyLike): string | null {
  const ctrl = event.ctrlKey || event.metaKey;
  const key = event.key;
  if (key === "F5") return "reload";
  if (key === "F11") return "fullscreen";
  if (!ctrl) return null;
  if (event.altKey) {
    if (key === "1") return "theme_auto";
    if (key === "2") return "theme_light";
    if (key === "3") return "theme_dark";
    return null;
  }
  if (event.shiftKey) {
    if (key === "?") return "show_shortcuts";
    if (key === "I" || key === "i") return "devtools";
    return null;
  }
  switch (key.toLowerCase()) {
    case "q":
      return "quit";
    case "w":
      return "hide";
    case "p":
      return "settings";
    case "0":
      return "zoom_reset";
    case "+":
    case "=":
      return "zoom_in";
    case "-":
      return "zoom_out";
    case "g":
      return "toggle_grid";
    case "j":
      return "downloads";
    case "u":
      return "add_account";
    case "m":
      return "open_chat_by_number";
    case "n":
      return "new_chat";
    case "/":
    case "?":
      return "show_shortcuts";
    default:
      return /^[1-9]$/.test(key) ? "account_" + key : null;
  }
}
