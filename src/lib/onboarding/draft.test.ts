import { describe, expect, it } from "vitest";

import { finishPatch, type Draft } from "./draft";

const draft: Draft = {
  interface_language: "system",
  theme: "auto",
  notifications: { enabled: true, show_photo: true, show_name: true, show_message: true, sound: true },
  tray_icon: true,
  notification_counter: true,
  quit_on_close: false,
  confirm_on_close: false,
  start_system: false,
  start_background: false,
  download_path: "",
  spellcheck: true,
  auto_grant: { microphone: false, camera: true, camera_microphone: false, screen_contents: false, screen_contents_audio: false },
};

describe("onboarding finish", () => {
  it("completes onboarding and keeps the choices", () => {
    const patch = finishPatch(draft) as Record<string, Record<string, unknown>>;
    expect(patch.onboarding).toEqual({ completed: true });
    expect(patch.system.quit_on_close).toBe(false);
    expect((patch.permissions.auto_grant as Record<string, boolean>).camera).toBe(true);
  });

  it("quits on close without a tray icon", () => {
    const patch = finishPatch({ ...draft, tray_icon: false }) as Record<string, Record<string, unknown>>;
    expect(patch.system.quit_on_close).toBe(true);
  });
});
