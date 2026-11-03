<script lang="ts">
  import { t, uiLocale } from "$lib/i18n/index.svelte";
  import type { UpdateInfo } from "$lib/ipc";

  // ZapZap's update popover, as a dialog: account pages would cover a popover.
  let { info, onclose }: { info: UpdateInfo; onclose: () => void } = $props();

  const released = $derived.by(() => {
    if (!info.published) return "";
    try {
      return new Intl.DateTimeFormat(uiLocale(), { dateStyle: "long" }).format(
        new Date(`${info.published}T12:00:00Z`),
      );
    } catch {
      return info.published;
    }
  });
</script>

<svelte:window
  onkeydowncapture={(e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }}
/>

<div class="backdrop">
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={t("Update available")}
  >
    <h2>{t("Update available")}</h2>
    <p>{t("Current version: {version}").replace("{version}", info.current)}</p>
    <p>{t("Latest version: {version}").replace("{version}", info.latest)}</p>
    {#if released}<p class="muted">{t("Released on {date}").replace("{date}", released)}</p>{/if}
    <div class="actions">
      <button class="btn" onclick={onclose}>{t("Close")}</button>
      {#if info.notes_url}
        <a class="btn" href={info.notes_url} target="_blank" rel="noreferrer">{t("Release notes")}</a>
      {/if}
      <a class="btn primary" href={info.download_url} target="_blank" rel="noreferrer">{t("Download")}</a>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 40%);
  }
  .dialog {
    width: min(400px, calc(100vw - 32px));
    padding: 20px 24px;
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
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
  a.btn {
    text-decoration: none;
  }
</style>
