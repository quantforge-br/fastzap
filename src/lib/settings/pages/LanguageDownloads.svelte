<script lang="ts">
  import { onMount } from "svelte";

  import Row from "$lib/components/settings/Row.svelte";
  import Section from "$lib/components/settings/Section.svelte";
  import SelectRow from "$lib/components/settings/SelectRow.svelte";
  import SwitchRow from "$lib/components/settings/SwitchRow.svelte";
  import { languageName, languages, t } from "$lib/i18n/index.svelte";
  import { app, type Settings } from "$lib/ipc";
  import { patchSettings, store } from "$lib/stores/settings.svelte";

  import Page from "../Page.svelte";

  const s = $derived(store.settings as Settings);
  const d = $derived(s.downloads);
  let defaultDir = $state("");

  onMount(() => {
    app.downloadsDefaultDir().then((dir) => (defaultDir = dir));
  });

  const setDownloads = (downloads: Record<string, unknown>) => patchSettings({ downloads });
  const languageOptions = $derived([
    { value: "system", label: t("System default") },
    ...languages.map((code) => ({ value: code, label: `${languageName(code)} - ${code}` })),
  ]);

  async function chooseFolder() {
    const folder = await app.pickFolder(d.download_path || defaultDir);
    if (folder) setDownloads({ download_path: folder });
  }
</script>

<Page
  title={t("Language and Download")}
  description={t("Manage interface language, downloads folder and spell checker.")}
>
  <Section title={t("Language")} description={t("Choose the interface language used by FastZap.")}>
    <SelectRow
      title={t("Interface language")}
      description={t("The interface language is applied immediately.")}
      value={s.system.interface_language}
      options={languageOptions}
      onchange={(interface_language) => patchSettings({ system: { interface_language } })}
    />
  </Section>

  <Section title={t("Downloads")} description={t("Choose where downloaded files are saved.")}>
    <SelectRow
      title={t("Download behavior")}
      description={t("Choose what happens when a download starts.")}
      value={d.behavior}
      options={[
        { value: "dialog", label: t("Show download window") },
        { value: "automatic", label: t("Save automatically to selected folder") },
        { value: "ask_every_time", label: t("Ask where to save every time") },
      ]}
      onchange={(behavior) => setDownloads({ behavior })}
    />
    <SelectRow
      title={t("PDF file clicks")}
      description={t(
        "Preview PDF files in WhatsApp or download them when the browser handles the PDF. WhatsApp's own preview may still open.",
      )}
      value={d.pdf_click_behavior}
      options={[
        { value: "preview", label: t("Preview in WhatsApp") },
        { value: "download", label: t("Download directly (where supported)") },
      ]}
      onchange={(pdf_click_behavior) => setDownloads({ pdf_click_behavior })}
    />
    <Row
      title={t("Download directory")}
      description={t("Set a custom folder or restore the default download location.")}
    >
      <input class="path" readonly value={d.download_path || defaultDir} />
      {#snippet control()}
        <button class="btn" onclick={chooseFolder}>{t("Define")}</button>
        <button class="btn" disabled={!d.download_path} onclick={() => setDownloads({ download_path: "" })}>
          {t("Restore")}
        </button>
      {/snippet}
    </Row>
    <SwitchRow
      title={t('Remember last folder when using "Save as"')}
      description={t("Only for the current account session.")}
      checked={d.remember_last_save_as_directory}
      onchange={(v) => setDownloads({ remember_last_save_as_directory: v })}
    />
    <SwitchRow
      title={t("Automatically open PDFs")}
      description={t("Open completed PDF downloads with the system default application.")}
      checked={d.auto_open_pdf}
      onchange={(auto_open_pdf) => setDownloads({ auto_open_pdf })}
    />
    <SwitchRow
      title={t("Automatically open images")}
      description={t("Open completed image downloads with the system default application.")}
      checked={d.auto_open_images}
      onchange={(auto_open_images) => setDownloads({ auto_open_images })}
    />
    <Row
      title={t("WhatsApp multiple-download permission")}
      description={t("Clear the remembered allow or block decision for repeated WhatsApp downloads.")}
    >
      {#snippet control()}
        <button
          class="btn"
          disabled={d.multiple_download_permission === "ask"}
          onclick={() => setDownloads({ multiple_download_permission: "ask" })}
        >
          {t("Reset")}
        </button>
      {/snippet}
    </Row>
  </Section>
</Page>

<style>
  .path {
    width: 100%;
    margin-top: 6px;
    box-sizing: border-box;
  }
</style>
