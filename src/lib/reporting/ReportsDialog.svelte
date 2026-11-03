<script lang="ts">
  import { onMount } from "svelte";

  import { t } from "$lib/i18n/index.svelte";
  import { reporting, type ReportView } from "$lib/ipc";

  import { kindLabel, previewText, statusLabel } from "./format";
  import { openReport } from "./request.svelte";

  // ZapZap's RecentReportsDialog.
  let { onclose }: { onclose: () => void } = $props();

  let reports = $state<ReportView[]>([]);
  let selected = $state<string | null>(null);
  const current = $derived(reports.find((r) => r.id === selected) ?? null);

  onMount(async () => {
    reports = await reporting.list();
    selected = reports[0]?.id ?? null;
  });

  async function remove(id: string) {
    await reporting.remove(id);
    reports = reports.filter((r) => r.id !== id);
    if (selected === id) selected = reports[0]?.id ?? null;
  }
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
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Local reports")}>
    <h2>{t("Local reports")}</h2>
    {#if reports.length === 0}
      <p class="muted">{t("No local reports.")}</p>
    {:else}
      <div class="split">
        <ul>
          {#each reports as report (report.id)}
            <li>
              <button class:active={report.id === selected} onclick={() => (selected = report.id)}>
                {report.created_at?.slice(0, 16).replace("T", " ")} — {kindLabel(report)} — {statusLabel(report.status)}
              </button>
            </li>
          {/each}
        </ul>
        {#if current}
          <div class="details">
            <pre>{previewText(current)}</pre>
            <p class="muted">{t("Complete report to copy")}</p>
            <pre>{current.markdown}</pre>
          </div>
        {/if}
      </div>
    {/if}
    <footer>
      {#if current}
        <button class="btn danger" onclick={() => current?.id && remove(current.id)}>{t("Delete")}</button>
      {/if}
      <span class="spacer"></span>
      <button class="btn" onclick={onclose}>{t("Close")}</button>
      <button
        class="btn primary"
        disabled={!current}
        onclick={() => {
          if (!current) return;
          openReport(current);
          onclose();
        }}>{t("Review and open selected report")}</button
      >
    </footer>
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
    display: flex;
    flex-direction: column;
    width: min(720px, calc(100vw - 32px));
    height: min(520px, calc(100vh - 32px));
    padding: 16px 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
    box-sizing: border-box;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 17px;
  }
  .split {
    display: grid;
    flex: 1;
    grid-template-columns: minmax(200px, 2fr) 3fr;
    gap: 12px;
    min-height: 0;
  }
  ul {
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }
  li button {
    width: 100%;
    padding: 6px;
    border: 0;
    border-radius: 4px;
    background: none;
    color: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  li button.active {
    background: var(--csr-mid);
  }
  .details {
    overflow-y: auto;
  }
  pre {
    margin: 0 0 8px;
    font-size: 12px;
    white-space: pre-wrap;
  }
  .muted {
    opacity: 0.7;
  }
  footer {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }
  .spacer {
    flex: 1;
  }
</style>
