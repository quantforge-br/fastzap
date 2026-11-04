<script lang="ts">
  import Section from "$lib/components/settings/Section.svelte";
  import SwitchRow from "$lib/components/settings/SwitchRow.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { Settings } from "$lib/ipc";
  import { patchSettings, store } from "$lib/stores/settings.svelte";

  import Page from "../Page.svelte";

  const n = $derived((store.settings as Settings).notifications);
  const set = (notifications: Record<string, unknown>) => patchSettings({ notifications });
  let presetsOpen = $state(false);

  const PRESETS = [
    { label: "Show everything", values: [true, true, true] },
    { label: "Hide content", values: [true, true, false] },
    { label: "Maximum privacy", values: [false, false, false] },
  ];

  function applyPreset(values: boolean[]) {
    presetsOpen = false;
    set({ show_photo: values[0], show_name: values[1], show_message: values[2] });
  }
</script>

<Page title={t("Notifications")} description={t("Configure notifications and privacy.")}>
  <Section title={t("Notifications")} description={t("Choose which information may appear in notifications.")}>
    {#snippet actions()}
      <div class="presets">
        <button
          class="btn"
          aria-label={t("Privacy presets")}
          aria-expanded={presetsOpen}
          disabled={!n.enabled}
          onclick={() => (presetsOpen = !presetsOpen)}
        >
          {t("Privacy")} ▾
        </button>
        {#if presetsOpen}
          <div class="menu" role="menu">
            {#each PRESETS as preset (preset.label)}
              <button role="menuitem" onclick={() => applyPreset(preset.values)}>{t(preset.label)}</button>
            {/each}
          </div>
        {/if}
      </div>
    {/snippet}
    <SwitchRow
      title={t("Desktop notifications")}
      description={t("Show WhatsApp notifications using the desktop notification system.")}
      checked={n.enabled}
      onchange={(enabled) => set({ enabled })}
    />
    <SwitchRow
      child
      disabled={!n.enabled}
      title={t("Contact photo")}
      description={t("Show the sender's photo when available.")}
      checked={n.show_photo}
      onchange={(show_photo) => set({ show_photo })}
    />
    <SwitchRow
      child
      disabled={!n.enabled}
      title={t("Contact name")}
      description={t("Show the sender or group name.")}
      checked={n.show_name}
      onchange={(show_name) => set({ show_name })}
    />
    <SwitchRow
      child
      disabled={!n.enabled}
      title={t("Message preview")}
      description={t("Show the content of the received message.")}
      checked={n.show_message}
      onchange={(show_message) => set({ show_message })}
    />
    <SwitchRow
      child
      disabled={!n.enabled}
      title={t("Notification sound")}
      description={t("Let the desktop play its alert sound for new messages.")}
      checked={n.sound}
      onchange={(sound) => set({ sound })}
    />
  </Section>
</Page>

<style>
  .presets {
    position: relative;
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 4px);
    z-index: 5;
    display: flex;
    flex-direction: column;
    min-width: 180px;
    padding: 4px;
    border: 1px solid var(--csr-mid);
    border-radius: 8px;
    background: var(--bg);
    box-shadow: 0 4px 16px rgb(0 0 0 / 20%);
  }
  .menu button {
    padding: 6px 10px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--csr-hover);
  }
</style>
