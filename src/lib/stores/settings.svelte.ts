// Shared reactive copy of the backend settings for the main UI.
import { events, on, settings as settingsApi, type Settings } from "$lib/ipc";

export const store = $state<{ settings: Settings | null }>({ settings: null });

let started = false;

/// Load the settings once and follow `settings://changed`.
export function startSettings() {
  if (started) return;
  started = true;
  settingsApi.get().then((s) => (store.settings = s));
  on<Settings>(events.settingsChanged, (s) => (store.settings = s));
}

/// Deep-merge `patch` into the stored settings (see `settings_patch`).
export async function patchSettings(patch: unknown): Promise<void> {
  store.settings = await settingsApi.patch(patch);
}
