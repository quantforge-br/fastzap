// Client-side decoration button themes (ZapZap's CSRButtonThemeProvider).

export type CsrButtonTheme =
  | "default"
  | "adwaita"
  | "plasma"
  | "ios"
  | "windows";

export interface CsrButtonDefinition {
  minimize: string;
  maximize: string;
  close: string;
  fontSize: number;
  width: number;
  height: number;
  radius: number;
}

export const CSR_BUTTON_THEMES: Record<CsrButtonTheme, CsrButtonDefinition> = {
  default: {
    minimize: "—",
    maximize: "□",
    close: "✕",
    fontSize: 14,
    width: 36,
    height: 28,
    radius: 6,
  },
  adwaita: {
    minimize: "–",
    maximize: "+",
    close: "×",
    fontSize: 14,
    width: 28,
    height: 28,
    radius: 14,
  },
  plasma: {
    minimize: "–",
    maximize: "▣",
    close: "✖",
    fontSize: 13,
    width: 32,
    height: 28,
    radius: 8,
  },
  ios: {
    minimize: "–",
    maximize: "+",
    close: "×",
    fontSize: 14,
    width: 28,
    height: 28,
    radius: 14,
  },
  windows: {
    minimize: "—",
    maximize: "□",
    close: "✕",
    fontSize: 12,
    width: 36,
    height: 28,
    radius: 2,
  },
};

/** The configured theme, or one matching the desktop (`XDG_CURRENT_DESKTOP`). */
export function resolveCsrButtonTheme(
  configured: string,
  desktop: string,
): CsrButtonTheme {
  const value = configured.trim().toLowerCase();
  if (value in CSR_BUTTON_THEMES) return value as CsrButtonTheme;
  const current = desktop.toLowerCase();
  if (current.includes("kde") || current.includes("plasma")) return "plasma";
  if (current.includes("gnome")) return "adwaita";
  return "default";
}
