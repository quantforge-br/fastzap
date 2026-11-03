<script lang="ts">
  // ZapZap's customization editor: file name and code, Ctrl+S saves.
  import { t } from "$lib/i18n/index.svelte";

  import type { AssetKind } from "./api";

  let {
    kind,
    title,
    name: initialName,
    content: initialContent,
    onsave,
    oncancel,
  }: {
    kind: AssetKind;
    title: string;
    name: string;
    content: string;
    onsave: (name: string, content: string) => Promise<string | null>;
    oncancel: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let name = $state(initialName);
  // svelte-ignore state_referenced_locally
  let content = $state(initialContent);
  let error = $state("");

  async function save() {
    error = (await onsave(name, content)) ?? "";
  }
</script>

<svelte:window
  onkeydowncapture={(e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      oncancel();
    } else if (e.ctrlKey && (e.key === "s" || e.key === "S")) {
      e.preventDefault();
      save();
    }
  }}
/>

<div class="backdrop">
  <div class="dialog" role="dialog" aria-modal="true" aria-label={title}>
    <h2>{title}</h2>
    <p class="muted">
      {kind === "css"
        ? t("Edit the stylesheet content and choose the file name.")
        : t("Edit the script content and choose the file name.")}
    </p>
    <label class="field">
      <span>{t("File name")}</span>
      <input bind:value={name} spellcheck="false" />
    </label>
    <label class="field grow">
      <span>{kind === "css" ? t("CSS code") : t("JavaScript code")}</span>
      <textarea bind:value={content} spellcheck="false" wrap="off"></textarea>
    </label>
    {#if error}<p class="info-box warning">{error}</p>{/if}
    <footer>
      <span class="muted">{t("Ctrl+S to save")}</span>
      <button class="btn" onclick={oncancel}>{t("Cancel")}</button>
      <button class="btn primary" onclick={save}>{t("Save")}</button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 45;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 40%);
  }
  .dialog {
    display: flex;
    flex-direction: column;
    width: min(780px, calc(100vw - 32px));
    height: min(560px, calc(100vh - 32px));
    padding: 16px 20px;
    box-sizing: border-box;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
  }
  h2 {
    margin: 0;
    font-size: 17px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 10px;
    font-size: 13px;
  }
  .grow {
    flex: 1;
    min-height: 0;
  }
  textarea {
    flex: 1;
    font-family: monospace;
    tab-size: 4;
    resize: none;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
  }
  footer .muted {
    margin-right: auto;
    font-size: 12px;
  }
</style>
