<script lang="ts">
  // ZapZap's "Multiple downloads" question and Download dialog.
  import { t } from "$lib/i18n/index.svelte";
  import { app, downloads, type DownloadDecision, type DownloadPrompt } from "$lib/ipc";

  let { prompt, ondone }: { prompt: DownloadPrompt; ondone: () => void } = $props();
  let moreOpen = $state(false);

  async function answer(decision: DownloadDecision) {
    await downloads.answer(prompt.id, decision);
    ondone();
  }
</script>

<svelte:window
  onkeydowncapture={(e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      // Esc blocks in the multiple-downloads question, as in ZapZap.
      answer(prompt.kind === "multiple" ? "block" : "cancel");
    }
  }}
/>

<div class="backdrop">
  {#if prompt.kind === "multiple"}
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="dl-title">
      <h2 id="dl-title">{t("Multiple downloads")}</h2>
      <p>{t("WhatsApp wants to download multiple files.")}</p>
      <p class="muted">{t("Do you want to allow these additional downloads?")}</p>
      <div class="actions">
        <button class="btn" onclick={() => answer("allow_once")}>{t("Allow once")}</button>
        <button class="btn" onclick={() => answer("always_allow")}>{t("Always allow")}</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn danger" autofocus onclick={() => answer("block")}>{t("Block")}</button>
      </div>
    </div>
  {:else}
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="dl-title">
      <h2 id="dl-title">{t("Download")}</h2>
      <p class="name">{prompt.name}</p>
      <p class="muted folder">{prompt.folder}</p>
      <div class="actions">
        <div class="more">
          <button class="btn" aria-haspopup="menu" aria-expanded={moreOpen} onclick={() => (moreOpen = !moreOpen)}>
            {t("More")} ▾
          </button>
          {#if moreOpen}
            <div class="menu" role="menu">
              <button role="menuitem" onclick={() => answer("save_as")}>{t("Save as")}</button>
              <button role="menuitem" onclick={() => ((moreOpen = false), app.openPath(prompt.folder))}>
                {t("Open folder")}
              </button>
              <hr />
              <button role="menuitem" onclick={() => answer("cancel")}>{t("Cancel")}</button>
            </div>
          {/if}
        </div>
        <button class="btn" onclick={() => answer("open")}>{t("Open")}</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn primary" autofocus onclick={() => answer("save")}>{t("Save")}</button>
      </div>
    </div>
  {/if}
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
  .dialog {
    width: min(440px, calc(100vw - 32px));
    padding: 16px 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
  }
  h2 {
    margin: 0 0 8px;
    font-size: 17px;
  }
  p {
    margin: 0 0 6px;
  }
  .name {
    font-weight: 600;
    word-break: break-word;
    user-select: text;
  }
  .folder {
    font-size: 12px;
    word-break: break-all;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
  .more {
    position: relative;
    margin-right: auto;
  }
  .menu {
    position: absolute;
    bottom: calc(100% + 4px);
    left: 0;
    display: flex;
    flex-direction: column;
    min-width: 160px;
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
  hr {
    width: 100%;
    margin: 4px 0;
    border: 0;
    border-top: 1px solid var(--csr-mid);
  }
</style>
