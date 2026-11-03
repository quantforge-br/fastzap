// First-run setup choices (ZapZap's InitialSetupModel), saved together on Finish.
import type { Settings } from "$lib/ipc";

export type Draft = {
  interface_language: string;
  theme: Settings["appearance"]["theme"];
  notifications: Pick<Settings["notifications"], "enabled" | "show_photo" | "show_name" | "show_message" | "sound">;
  tray_icon: boolean;
  notification_counter: boolean;
  quit_on_close: boolean;
  confirm_on_close: boolean;
  start_system: boolean;
  start_background: boolean;
  download_path: string;
  spellcheck: boolean;
  auto_grant: Pick<
    Settings["permissions"]["auto_grant"],
    "microphone" | "camera" | "camera_microphone" | "screen_contents" | "screen_contents_audio"
  >;
};

export function draftFrom(s: Settings): Draft {
  const n = s.notifications;
  const g = s.permissions.auto_grant;
  return {
    interface_language: s.system.interface_language,
    theme: s.appearance.theme,
    notifications: {
      enabled: n.enabled,
      show_photo: n.show_photo,
      show_name: n.show_name,
      show_message: n.show_message,
      sound: n.sound,
    },
    tray_icon: s.appearance.tray_icon,
    notification_counter: s.appearance.notification_counter,
    quit_on_close: s.system.quit_on_close,
    confirm_on_close: s.system.confirm_on_close,
    start_system: s.system.start_system,
    start_background: s.system.start_background,
    download_path: s.downloads.download_path,
    spellcheck: s.spellcheck.enabled,
    auto_grant: {
      microphone: g.microphone,
      camera: g.camera,
      camera_microphone: g.camera_microphone,
      screen_contents: g.screen_contents,
      screen_contents_audio: g.screen_contents_audio,
    },
  };
}

/** The settings patch for Finish; without a tray icon closing must quit. */
export function finishPatch(d: Draft): Record<string, unknown> {
  return {
    system: {
      interface_language: d.interface_language,
      quit_on_close: d.tray_icon ? d.quit_on_close : true,
      confirm_on_close: d.confirm_on_close,
      start_system: d.start_system,
      start_background: d.start_background,
    },
    appearance: { theme: d.theme, tray_icon: d.tray_icon, notification_counter: d.notification_counter },
    notifications: { ...d.notifications },
    downloads: { download_path: d.download_path },
    spellcheck: { enabled: d.spellcheck },
    permissions: { auto_grant: { ...d.auto_grant } },
    onboarding: { completed: true },
  };
}
