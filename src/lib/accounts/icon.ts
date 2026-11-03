// Account icon values, compatible with ZapZap's persisted `users.icon`
// (mirrors src-tauri/src/accounts/icon.rs).
import DEFAULT_ICON from "./default-icon.svg?raw";

export { DEFAULT_ICON };

export const PHOTO_PREFIX = "data:image/png;base64,";
export const PROFILE_PREFIX = "zapzap-account-image:v1:";
export const PHOTO_SIZE = 256;

export interface IconChoice {
  /** Generated SVG icon. */
  icon: string;
  /** Embedded PNG photo, if any. */
  photo: string | null;
  usePhoto: boolean;
}

function randomColor(random: () => number): string {
  const c = () => Math.floor(random() * 256);
  return `rgb(${c()}, ${c()}, ${c()})`;
}

/** ZapZap's default icon with new random gradient colors. */
export function newIconSvg(random: () => number = Math.random): string {
  return DEFAULT_ICON.replace("#209232", randomColor(random)).replace("#34c640", randomColor(random));
}

/** Split a stored icon value into its generated icon and photo. */
export function parseIcon(value: string | null): IconChoice {
  if (value?.startsWith(PROFILE_PREFIX)) {
    try {
      const p = JSON.parse(value.slice(PROFILE_PREFIX.length));
      if (
        (p?.type === "icon" || p?.type === "photo") &&
        typeof p.icon === "string" &&
        typeof p.photo === "string" &&
        p.photo.startsWith(PHOTO_PREFIX)
      ) {
        return { icon: p.icon, photo: p.photo, usePhoto: p.type === "photo" };
      }
    } catch {
      // Invalid profiles fall back to the default icon, as in ZapZap.
    }
    return { icon: DEFAULT_ICON, photo: null, usePhoto: false };
  }
  if (value?.startsWith(PHOTO_PREFIX)) return { icon: DEFAULT_ICON, photo: value, usePhoto: true };
  return { icon: value || DEFAULT_ICON, photo: null, usePhoto: false };
}

/** Stored value for a choice, keeping both images when there is a photo. */
export function serializeIcon(choice: IconChoice): string {
  if (!choice.photo) return choice.icon || DEFAULT_ICON;
  return (
    PROFILE_PREFIX +
    JSON.stringify({ type: choice.usePhoto ? "photo" : "icon", icon: choice.icon || DEFAULT_ICON, photo: choice.photo })
  );
}

/** An <img> source for a stored icon value. */
export function iconSrc(value: string | null): string {
  const choice = parseIcon(value);
  if (choice.usePhoto && choice.photo) return choice.photo;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(choice.icon)}`;
}

/** Decode a base64 data URL without fetch, which the CSP may block. */
export function dataUrlToBlob(url: string): Blob {
  const [head, data] = url.split(",", 2);
  const type = /^data:([^;]+)/.exec(head)?.[1] ?? "application/octet-stream";
  return new Blob([Uint8Array.from(atob(data), (c) => c.charCodeAt(0))], { type });
}

/** Crop an image to a centered circle and encode it as a 256 px PNG data URL. */
export async function photoFromBlob(blob: Blob): Promise<string> {
  const bitmap = await createImageBitmap(blob);
  const side = Math.min(bitmap.width, bitmap.height);
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = PHOTO_SIZE;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("canvas unavailable");
  ctx.beginPath();
  ctx.arc(PHOTO_SIZE / 2, PHOTO_SIZE / 2, PHOTO_SIZE / 2, 0, Math.PI * 2);
  ctx.clip();
  ctx.drawImage(
    bitmap,
    (bitmap.width - side) / 2,
    (bitmap.height - side) / 2,
    side,
    side,
    0,
    0,
    PHOTO_SIZE,
    PHOTO_SIZE,
  );
  bitmap.close();
  return canvas.toDataURL("image/png");
}
