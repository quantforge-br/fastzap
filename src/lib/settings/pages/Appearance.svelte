<script lang="ts">
  import { onMount } from "svelte";

  import Row from "$lib/components/settings/Row.svelte";
  import Section from "$lib/components/settings/Section.svelte";
  import SelectRow from "$lib/components/settings/SelectRow.svelte";
  import SwitchRow from "$lib/components/settings/SwitchRow.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { app, type Settings } from "$lib/ipc";
  import { patchSettings, store } from "$lib/stores/settings.svelte";

  import Page from "../Page.svelte";

  const s = $derived(store.settings as Settings);
  const a = $derived(s.appearance);
  const panel = $derived(a.floating_panel);
  let positioning = $state(true);

  onMount(() => {
    app.panelSupportsPositioning().then((v) => (positioning = v));
  });

  const set = (appearance: Record<string, unknown>) => patchSettings({ appearance });
  const setPanel = (floating_panel: Record<string, unknown>) => set({ floating_panel });
  const scales = Array.from({ length: 31 }, (_, i) => 50 + i * 5).map((v) => ({
    value: v,
    label: `${v} %`,
  }));
  const waylandNote = "Your Wayland desktop controls window placement.";

  async function compactChatList() {
    await set({ resizable_chat_list_enabled: true });
    await app.action("compact_chat_list");
  }
</script>

<Page
  title={t("Appearance")}
  description={t("Adjust interface chrome, quick access, theme, tray icon, grid view, and window decorations.")}
>
  <Section title={t("Interface")} description={t("Show or hide primary application chrome.")}>
    <SwitchRow
      title={t("Browser sidebar")}
      description={t("Show account navigation in the browser shell.")}
      checked={a.sidebar}
      onchange={(sidebar) => set({ sidebar })}
    />
    <SwitchRow
      title={t("Menu bar")}
      description={t("Show the main window menu bar.")}
      checked={a.menubar}
      onchange={(menubar) => set({ menubar })}
    />
    <SwitchRow
      title={t("Resizable chat list")}
      description={t(
        "Drag the edge of the WhatsApp Web chat list to change its width. Double-click the edge to restore the default width.",
      )}
      checked={a.resizable_chat_list_enabled}
      onchange={(v) => set({ resizable_chat_list_enabled: v })}
    />
    <Row
      title={t("Compact chat list")}
      description={t("Set the chat list to 85 px. You can resize it again afterwards.")}
    >
      {#snippet control()}
        <button class="btn" onclick={compactChatList}>{t("Apply")}</button>
      {/snippet}
    </Row>
    <SelectRow
      title={t("Interface scale")}
      description={t("Scale the interface for high-DPI or accessibility needs.")}
      value={a.scale}
      options={scales}
      onchange={(scale) => set({ scale })}
    />
  </Section>

  <Section
    title={t("Quick Access")}
    description={t(
      "Control the floating monitoring panel and the account selector integrated into WhatsApp Web. Both are independent and can be shown at the same time.",
    )}
  >
    <SwitchRow
      title={t("Floating monitoring panel")}
      description={t("Monitor accounts in a separate window. Closing it only hides it temporarily.")}
      checked={panel.enabled}
      onchange={(enabled) => setPanel({ enabled })}
    />
    {#if panel.enabled}
      <SelectRow
        child
        title={t("Show panel")}
        description={t("Choose when the panel appears automatically.")}
        value={panel.mode}
        options={[
          { value: "always", label: t("Always") },
          { value: "when_hidden", label: t("When the main window is hidden") },
          { value: "on_demand", label: t("On demand") },
        ]}
        onchange={(mode) => setPanel({ mode })}
      />
      <SwitchRow
        child
        title={t("Keep above other windows")}
        description={t("Keep the panel visible while working in other applications.")}
        checked={panel.on_top}
        onchange={(on_top) => setPanel({ on_top })}
      />
      <SwitchRow
        child
        title={t("Remember position")}
        description={positioning
          ? t("Restore the panel position the next time FastZap starts.")
          : t(waylandNote)}
        disabled={!positioning}
        checked={panel.remember_position}
        onchange={(remember_position) => setPanel({ remember_position })}
      />
      <SwitchRow
        child
        title={t("Lock position")}
        description={positioning
          ? t("Prevent dragging. Unlock directly in the panel to move it again.")
          : t(waylandNote)}
        disabled={!positioning}
        checked={panel.lock_position}
        onchange={(lock_position) => setPanel({ lock_position })}
      />
      <Row child title={t("Show monitoring panel")} description={t("Reopen the panel without changing its display mode.")}>
        {#snippet control()}
          <button class="btn" onclick={() => app.action("show_panel")}>{t("Show")}</button>
        {/snippet}
      </Row>
      <Row
        child
        disabled={!positioning}
        title={t("Reset panel position")}
        description={t("Bring the panel back into the available screen area.")}
      >
        {#snippet control()}
          <button class="btn" onclick={() => setPanel({ position: null })}>{t("Reset")}</button>
        {/snippet}
      </Row>
    {/if}
    <SwitchRow
      title={t("Integrated account selector")}
      description={t("Show a compact account selector integrated into WhatsApp Web.")}
      checked={a.integrated_account_selector_enabled}
      onchange={(v) => set({ integrated_account_selector_enabled: v })}
    />
  </Section>

  <Section title={t("Theme")} description={t("Choose the visual theme.")}>
    <SelectRow
      title={t("Theme")}
      value={a.theme}
      options={[
        { value: "auto", label: t("Automatic") },
        { value: "light", label: t("Light") },
        { value: "dark", label: t("Dark") },
      ]}
      onchange={(theme) => set({ theme })}
    />
  </Section>

  <Section title={t("Tray icon")} description={t("Control tray icon visibility, style, and unread counter.")}>
    <SwitchRow
      title={t("Show tray icon")}
      description={t("Display FastZap in the system notification area.")}
      checked={a.tray_icon}
      onchange={(tray_icon) => set({ tray_icon })}
    />
    <SwitchRow
      child
      disabled={!a.tray_icon}
      title={t("Unread counter")}
      description={t("Show the number of unread messages.")}
      checked={a.notification_counter}
      onchange={(notification_counter) => set({ notification_counter })}
    />
    <SelectRow
      child
      disabled={!a.tray_icon}
      title={t("Icon style")}
      value={a.tray_theme}
      options={[
        { value: "default", label: t("Default") },
        { value: "symbolic_light", label: t("Symbolic light") },
        { value: "symbolic_dark", label: t("Symbolic dark") },
      ]}
      onchange={(tray_theme) => set({ tray_theme })}
    />
  </Section>

  <Section title={t("Grid view")} description={t("Choose how many columns are used by grid view.")}>
    <SelectRow
      title={t("Grid columns")}
      description={t("Number of account columns in grid view.")}
      value={a.grid_cols}
      options={[2, 3, 4].map((v) => ({ value: v, label: String(v) }))}
      onchange={(grid_cols) => set({ grid_cols })}
    />
  </Section>

  <Section title={t("Window decoration")} description={t("Customize the window controls drawn by FastZap.")}>
    <SwitchRow
      title={t("Use custom decoration")}
      description={t("Use window controls drawn by FastZap.")}
      checked={a.csr}
      onchange={(csr) => set({ csr })}
    />
    <SelectRow
      child
      disabled={!a.csr}
      title={t("Button style")}
      description={t("Visual style used by custom window buttons.")}
      value={a.csr_button_theme}
      options={[
        { value: "default", label: t("Default") },
        { value: "adwaita", label: "Adwaita" },
        { value: "plasma", label: "Plasma" },
        { value: "ios", label: "iOS" },
        { value: "windows", label: "Windows" },
      ]}
      onchange={(csr_button_theme) => set({ csr_button_theme })}
    />
    <SwitchRow
      child
      disabled={!a.csr}
      title={t("Minimize")}
      checked={a.csr_show_minimize_button}
      onchange={(v) => set({ csr_show_minimize_button: v })}
    />
    <SwitchRow
      child
      disabled={!a.csr}
      title={t("Maximize")}
      checked={a.csr_show_maximize_button}
      onchange={(v) => set({ csr_show_maximize_button: v })}
    />
    <SelectRow
      child
      disabled={!a.csr}
      title={t("Button position")}
      description={t("Place window buttons on the right or left.")}
      value={a.csr_buttons_direction}
      options={[
        { value: "right", label: t("Right") },
        { value: "left", label: t("Left") },
      ]}
      onchange={(csr_buttons_direction) => set({ csr_buttons_direction })}
    />
  </Section>
</Page>
