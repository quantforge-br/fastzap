<script lang="ts">
  import { onMount } from "svelte";

  import Row from "$lib/components/settings/Row.svelte";
  import Section from "$lib/components/settings/Section.svelte";
  import SwitchRow from "$lib/components/settings/SwitchRow.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { app, type Settings } from "$lib/ipc";
  import { patchSettings, store } from "$lib/stores/settings.svelte";

  import Page from "../Page.svelte";

  type Key = keyof Settings["permissions"]["auto_grant"];
  const GROUPS: { title: string; items: { id: Key; label: string; description: string }[] }[] = [
    {
      title: "Device access",
      items: [
        { id: "microphone", label: "Microphone", description: "Automatically allow access to your microphone." },
        { id: "camera", label: "Camera", description: "Automatically allow access to your camera." },
        {
          id: "camera_microphone",
          label: "Camera and microphone",
          description: "Automatically allow simultaneous access to the camera and microphone.",
        },
        { id: "location", label: "Location", description: "Automatically allow access to your location." },
      ],
    },
    {
      title: "Sharing",
      items: [
        { id: "screen_contents", label: "Screen sharing", description: "Automatically allow sharing of screen contents." },
        {
          id: "screen_contents_audio",
          label: "Screen with audio",
          description: "Automatically allow screen sharing with audio.",
        },
      ],
    },
    {
      title: "Advanced",
      items: [
        {
          id: "mouse_lock",
          label: "Pointer lock",
          description: "Automatically allow the page to capture the mouse pointer.",
        },
      ],
    },
  ];
  const ALL = GROUPS.flatMap((g) => g.items.map((i) => i.id));
  const FLATPAK_COMMAND = "flatpak override --user --filesystem=home io.github.quantmind_br.fastzap";

  const grants = $derived((store.settings as Settings).permissions.auto_grant);
  let flatpak = $state(false);

  onMount(() => {
    app.info().then((info) => (flatpak = info.flatpak));
  });

  const setAll = (value: boolean) =>
    patchSettings({ permissions: { auto_grant: Object.fromEntries(ALL.map((id) => [id, value])) } });
</script>

<Page
  title={t("Permissions")}
  description={t("Define which permissions can be granted automatically to WhatsApp Web.")}
>
  <p class="info-box">{t("Disabled permissions will continue to be requested when needed.")}</p>
  <div class="bulk">
    <button class="btn" disabled={ALL.every((id) => grants[id])} onclick={() => setAll(true)}>
      {t("Allow all")}
    </button>
    <button class="btn" disabled={ALL.every((id) => !grants[id])} onclick={() => setAll(false)}>
      {t("Remove all")}
    </button>
  </div>
  {#each GROUPS as group (group.title)}
    <Section title={t(group.title)}>
      {#each group.items as item (item.id)}
        <SwitchRow
          title={t(item.label)}
          description={t(item.description)}
          checked={grants[item.id]}
          onchange={(v) => patchSettings({ permissions: { auto_grant: { [item.id]: v } } })}
        />
      {/each}
    </Section>
  {/each}

  {#if flatpak}
    <Section
      title={t("Flatpak permissions")}
      description={t(
        "Grant filesystem access if downloads, imports, or dictionaries cannot reach folders outside the sandbox.",
      )}
    >
      <Row
        title={t("Flatpak sandbox: if file access fails, grant folder permissions using Flatseal or flatpak override.")}
      >
        <input class="command" readonly value={FLATPAK_COMMAND} />
        {#snippet control()}
          <button class="btn" onclick={() => navigator.clipboard.writeText(FLATPAK_COMMAND)}>
            {t("Copy")}
          </button>
        {/snippet}
      </Row>
      <Row
        title="Flatseal"
        description={t(
          "Flatseal is a graphical utility to review and modify permissions from your Flatpak applications.",
        )}
      >
        {#snippet control()}
          <a class="btn" href="https://flathub.org/apps/com.github.tchx84.Flatseal" target="_blank" rel="noreferrer">
            {t("Install Flatseal on Linux | Flathub")}
          </a>
        {/snippet}
      </Row>
    </Section>
  {/if}
</Page>

<style>
  .bulk {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    margin-bottom: 12px;
  }
  .command {
    width: 100%;
    margin-top: 6px;
    box-sizing: border-box;
    font-family: monospace;
  }
  a.btn {
    text-decoration: none;
  }
</style>
