<script lang="ts">
  import { onMount } from "svelte";

  import Section from "$lib/components/settings/Section.svelte";
  import SelectRow from "$lib/components/settings/SelectRow.svelte";
  import SwitchRow from "$lib/components/settings/SwitchRow.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { app, type Settings } from "$lib/ipc";
  import { patchSettings, store } from "$lib/stores/settings.svelte";

  import Page from "../Page.svelte";

  const sys = $derived((store.settings as Settings).system);
  // The display backend is read before the window exists.
  const initialBackend = (store.settings as Settings).system.display_backend;
  let flatpak = $state(true);

  onMount(() => {
    app.info().then((info) => (flatpak = info.flatpak));
  });

  const set = (system: Record<string, unknown>) => patchSettings({ system });
</script>

<Page
  title={t("System and startup")}
  description={t("Configure how FastZap starts, behaves in the system, and integrates with Linux.")}
  restart={sys.display_backend !== initialBackend}
>
  <Section title={t("Startup")} description={t("Define how FastZap behaves when your session starts.")}>
    <SwitchRow
      title={t("Start with the system")}
      description={t("Open FastZap automatically when the session starts.")}
      checked={sys.start_system}
      onchange={(start_system) => set({ start_system })}
    />
    <SwitchRow
      title={t("Start in the background")}
      description={t("Open without showing the main window.")}
      checked={sys.start_background}
      onchange={(start_background) => set({ start_background })}
    />
  </Section>
  <Section title={t("Window")} description={t("Define what happens when the main window is closed.")}>
    <SelectRow
      title={t("Close behavior")}
      description={t("Choose whether FastZap continues in the background or quits.")}
      value={sys.quit_on_close ? "quit_application" : "keep_running"}
      options={[
        { value: "keep_running", label: t("Continue in the background") },
        { value: "quit_application", label: t("Quit FastZap") },
      ]}
      onchange={(v) => set({ quit_on_close: v === "quit_application" })}
    />
    <SwitchRow
      child
      disabled={!sys.quit_on_close}
      title={t("Confirm before quitting")}
      description={t("Ask for confirmation before completely closing FastZap.")}
      checked={sys.confirm_on_close}
      onchange={(confirm_on_close) => set({ confirm_on_close })}
    />
  </Section>
  <Section title={t("Linux integration")} description={t("Settings related to the desktop environment and platform.")}>
    <SwitchRow
      title={t("System file dialogs")}
      description={t("Use the file picker provided by the desktop environment.")}
      checked={!sys.dont_use_native_dialog}
      onchange={(v) => set({ dont_use_native_dialog: !v })}
    />
    {#if !flatpak}
      <SelectRow
        title={t("Display backend")}
        description={t(
          "Automatic is recommended and uses the native backend for the current session. X11 is available as a compatibility fallback.",
        )}
        value={sys.display_backend}
        options={[
          { value: "auto", label: t("Automatic") },
          { value: "wayland", label: "Wayland" },
          { value: "xcb", label: "X11 / XWayland" },
        ]}
        onchange={(display_backend) => set({ display_backend })}
      />
    {/if}
  </Section>
</Page>
