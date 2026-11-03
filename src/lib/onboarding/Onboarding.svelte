<script lang="ts">
  // First-run setup (ZapZap's InitialSetupDialog): six steps, saved on Finish.
  import { onMount } from "svelte";

  import Row from "$lib/components/settings/Row.svelte";
  import Section from "$lib/components/settings/Section.svelte";
  import SelectRow from "$lib/components/settings/SelectRow.svelte";
  import SwitchRow from "$lib/components/settings/SwitchRow.svelte";
  import { languageName, languages, t } from "$lib/i18n/index.svelte";
  import { app, spellcheck, type Settings, type SpellState } from "$lib/ipc";
  import { dictionaryLabel } from "$lib/spellcheck/format";
  import { patchSettings } from "$lib/stores/settings.svelte";

  import { draftFrom, finishPatch } from "./draft";

  let { settings, onclose }: { settings: Settings; onclose: () => void } = $props();

  const STEPS = ["Basics", "Notifications", "Background", "Files", "Permissions", "Finish"];
  const APP_ID = "io.github.quantmind_br.fastzap";
  const FLATPAK_COMMAND = `flatpak override --user --filesystem=home ${APP_ID}`;

  let step = $state(0);
  // svelte-ignore state_referenced_locally
  let draft = $state(draftFrom(settings));
  let spell = $state<SpellState | null>(null);
  let flatpak = $state(false);
  let defaultDir = $state("");
  let copied = $state(false);
  let saving = $state(false);

  onMount(() => {
    spellcheck.state().then((s) => (spell = s));
    app.info().then((info) => (flatpak = info.flatpak));
    app.downloadsDefaultDir().then((dir) => (defaultDir = dir));
  });

  const languageOptions = $derived([
    { value: "system", label: t("System default") },
    ...languages.map((code) => ({ value: code, label: `${languageName(code)} - ${code}` })),
  ]);
  const dictionaries = $derived(spell?.languages.map(dictionaryLabel).join(", ") ?? "");

  async function chooseFolder() {
    const folder = await app.pickFolder(draft.download_path || defaultDir);
    if (folder) draft.download_path = folder;
  }

  async function copyCommand() {
    try {
      await navigator.clipboard.writeText(FLATPAK_COMMAND);
      copied = true;
    } catch {
      copied = false;
    }
  }

  // Skip keeps the current settings and does not ask again.
  async function skip() {
    await patchSettings({ onboarding: { completed: true } });
    onclose();
  }

  async function finish() {
    saving = true;
    try {
      await patchSettings(finishPatch(draft));
      onclose();
    } finally {
      saving = false;
    }
  }
</script>

<div class="backdrop">
  <div class="setup" role="dialog" aria-modal="true" aria-label={t("Setup FastZap")}>
    <nav>
      <h1>{t("Welcome to FastZap")}</h1>
      <p class="muted">{t("Choose the essentials now. You can change everything later in Settings.")}</p>
      <ol>
        {#each STEPS as name, i (name)}
          <li>
            <button class:current={i === step} aria-current={i === step ? "step" : undefined} onclick={() => (step = i)}>
              <span class="index">{i + 1}</span>{t(name)}
            </button>
          </li>
        {/each}
      </ol>
    </nav>

    <div class="body">
      <div class="page">
        {#if step === 0}
          <h2>{t("Language and appearance")}</h2>
          <p class="muted">{t("Start with the interface language and a comfortable visual theme.")}</p>
          <Section title={t("Language")} description={t("Choose the interface language used by FastZap.")}>
            <SelectRow
              title={t("Interface language")}
              description={t("The interface language is applied when you finish setup.")}
              value={draft.interface_language}
              options={languageOptions}
              onchange={(v) => (draft.interface_language = v)}
            />
          </Section>
          <Section title={t("Theme")} description={t("Choose the visual theme.")}>
            <SelectRow
              title={t("Theme")}
              value={draft.theme}
              options={[
                { value: "auto", label: t("Automatic") },
                { value: "light", label: t("Light") },
                { value: "dark", label: t("Dark") },
              ]}
              onchange={(v) => (draft.theme = v)}
            />
          </Section>
        {:else if step === 1}
          <h2>{t("Notifications and privacy")}</h2>
          <p class="muted">{t("Decide how much message information appears in native notification banners.")}</p>
          <Section title={t("Desktop notifications")} description={t("Choose whether FastZap may show desktop notifications.")}>
            <SwitchRow
              title={t("Enable notifications")}
              description={t("Allow FastZap to publish native desktop notifications for WhatsApp activity.")}
              checked={draft.notifications.enabled}
              onchange={(v) => (draft.notifications.enabled = v)}
            />
          </Section>
          <Section title={t("Notification privacy")} description={t("Limit what is visible in notification banners.")}>
            {#each [["show_photo", "Show contact photo", "Display the sender avatar when it is available."], ["show_name", "Show contact name", "Display the sender or group name."], ["show_message", "Show message preview", "Display the message text in the notification."], ["sound", "Notification sound", "Let the desktop play its alert sound for new messages."]] as const as [key, title, description] (key)}
              <SwitchRow
                title={t(title)}
                description={t(description)}
                disabled={!draft.notifications.enabled}
                checked={draft.notifications[key]}
                onchange={(v) => (draft.notifications[key] = v)}
              />
            {/each}
          </Section>
        {:else if step === 2}
          <h2>{t("Background behavior")}</h2>
          <p class="muted">{t("Choose whether FastZap should stay available from the tray and start with your session.")}</p>
          <Section title={t("Tray icon")} description={t("Control tray icon visibility and unread counter.")}>
            <SwitchRow
              title={t("Enable tray icon")}
              description={t("Show FastZap in the system tray.")}
              checked={draft.tray_icon}
              onchange={(v) => (draft.tray_icon = v)}
            />
            <SwitchRow
              child
              disabled={!draft.tray_icon}
              title={t("Notification counter")}
              description={t("Show unread notifications on the tray icon.")}
              checked={draft.notification_counter}
              onchange={(v) => (draft.notification_counter = v)}
            />
          </Section>
          <Section title={t("Window behavior")} description={t("Configure close behavior.")}>
            <SelectRow
              title={t("Close behavior")}
              description={t("Choose whether FastZap continues in the background or quits.")}
              disabled={!draft.tray_icon}
              value={!draft.tray_icon || draft.quit_on_close ? "quit" : "background"}
              options={[
                { value: "background", label: t("Continue in the background") },
                { value: "quit", label: t("Quit FastZap") },
              ]}
              onchange={(v) => (draft.quit_on_close = v === "quit")}
            />
            <SwitchRow
              child
              disabled={draft.tray_icon && !draft.quit_on_close}
              title={t("Confirm before quitting")}
              description={t("Ask for confirmation before completely closing FastZap.")}
              checked={draft.confirm_on_close}
              onchange={(v) => (draft.confirm_on_close = v)}
            />
          </Section>
          <Section title={t("Startup")} description={t("Control how FastZap behaves when your desktop session starts.")}>
            <SwitchRow
              title={t("Start with the system")}
              description={t("Create or remove the desktop autostart entry.")}
              checked={draft.start_system}
              onchange={(v) => (draft.start_system = v)}
            />
            <SwitchRow
              child
              title={t("Start minimized")}
              description={t("Open FastZap in the background instead of showing the main window.")}
              checked={draft.start_background}
              onchange={(v) => (draft.start_background = v)}
            />
          </Section>
        {:else if step === 3}
          <h2>{t("Downloads and typing")}</h2>
          <p class="muted">{t("Choose where files are saved and enable spell checking when dictionaries are available.")}</p>
          <Section title={t("Downloads")} description={t("Choose where downloaded files are saved.")}>
            <Row
              title={t("Download directory")}
              description={draft.download_path || defaultDir || t("Set a custom folder or keep the current download location.")}
            >
              {#snippet control()}
                <button class="btn" onclick={chooseFolder}>{t("Choose folder")}</button>
              {/snippet}
            </Row>
          </Section>
          {#if spell?.supported !== false}
            <Section title={t("Spell checker")} description={t("Select Hunspell dictionaries and the languages used for spell checking.")}>
              <SwitchRow
                title={t("Enable spell checker")}
                description={dictionaries || t("No dictionary available")}
                checked={draft.spellcheck}
                onchange={(v) => (draft.spellcheck = v)}
              />
            </Section>
          {/if}
        {:else if step === 4}
          <h2>{t("Essential permissions")}</h2>
          <p class="muted">{t("Allow only the WhatsApp Web permissions you want to grant automatically.")}</p>
          <Section title={t("Permissions")} description={t("When an option is unchecked, FastZap will ask again in each session.")}>
            {#each [["microphone", "Microphone", "Automatically allow access to your microphone."], ["camera", "Camera", "Automatically allow access to your camera."], ["camera_microphone", "Camera and microphone", "Automatically allow simultaneous access to the camera and microphone."], ["screen_contents", "Screen sharing", "Automatically allow sharing of screen contents."], ["screen_contents_audio", "Screen with audio", "Automatically allow screen sharing with audio."]] as const as [key, title, description] (key)}
              <SwitchRow
                title={t(title)}
                description={t(description)}
                checked={draft.auto_grant[key]}
                onchange={(v) => (draft.auto_grant[key] = v)}
              />
            {/each}
          </Section>
          {#if flatpak}
            <Section
              title={t("Flatpak permissions")}
              description={t("Grant filesystem access if downloads, imports, or dictionaries cannot reach folders outside the sandbox.")}
            >
              <div class="info-box warning">
                {t("Flatpak sandbox: if file access fails, grant folder permissions using Flatseal or flatpak override.")}
              </div>
              <div class="command">
                <input readonly value={FLATPAK_COMMAND} title={t("Select and copy this command in your terminal")} />
                <button class="btn" onclick={copyCommand}>{copied ? t("Copied") : t("Copy")}</button>
              </div>
            </Section>
          {/if}
        {:else}
          <h2>{t("You're ready")}</h2>
          <p class="muted">{t("FastZap will save these preferences now. Advanced options remain available in Settings.")}</p>
          <Section title={t("Finish setup")} description={t("Review advanced options later whenever you need them.")}>
            <div class="info-box">
              {t("You can revisit all choices from Settings at any time. Proxy, custom CSS/JavaScript, Flatpak permissions, and performance tweaks are still available there.")}
            </div>
          </Section>
        {/if}
      </div>

      <footer>
        <button class="btn" onclick={skip}>{t("Skip setup")}</button>
        <span class="spacer"></span>
        <button class="btn" disabled={step === 0} onclick={() => step--}>{t("Back")}</button>
        {#if step < STEPS.length - 1}
          <button class="btn primary" onclick={() => step++}>{t("Next")}</button>
        {:else}
          <button class="btn primary" disabled={saving} onclick={finish}>{t("Finish")}</button>
        {/if}
      </footer>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 40%);
  }
  .setup {
    display: flex;
    width: min(900px, calc(100vw - 32px));
    height: min(620px, calc(100vh - 32px));
    border: 1px solid var(--csr-mid);
    border-radius: 18px;
    background: var(--bg);
    overflow: hidden;
  }
  nav {
    flex: 0 0 220px;
    padding: 20px 16px;
    background: var(--sidebar-bg);
    overflow-y: auto;
  }
  nav h1 {
    margin: 0 0 6px;
    font-size: 18px;
  }
  ol {
    margin: 16px 0 0;
    padding: 0;
    list-style: none;
  }
  ol button {
    display: flex;
    gap: 10px;
    align-items: center;
    width: 100%;
    padding: 8px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  ol button.current {
    background: var(--bg);
    font-weight: 600;
  }
  .index {
    display: inline-grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    font-size: 12px;
  }
  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .page {
    flex: 1;
    padding: 20px 24px;
    overflow-y: auto;
  }
  .page h2 {
    margin: 0 0 4px;
    font-size: 20px;
  }
  footer {
    display: flex;
    gap: 8px;
    padding: 12px 24px;
    border-top: 1px solid var(--csr-mid);
  }
  .spacer {
    flex: 1;
  }
  .command {
    display: flex;
    gap: 8px;
    padding: 8px 0;
  }
  .command input {
    flex: 1;
    font-family: monospace;
  }
  @media (max-width: 640px) {
    nav {
      display: none;
    }
  }
</style>
