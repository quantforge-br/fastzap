<script lang="ts">
  // Download history (ZapZap's DownloadsWindow, Ctrl+J).
  import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
  import { onMount } from "svelte";

  import { t } from "$lib/i18n/index.svelte";
  import { app, downloads, events, on, type DownloadItem, type Settings } from "$lib/ipc";
  import { store } from "$lib/stores/settings.svelte";

  import { elideName } from "./format";

  let { onclose, onsettings }: { onclose: () => void; onsettings: () => void } = $props();
  let items = $state<DownloadItem[]>([]);

  const STATUS: Record<DownloadItem["status"], string> = {
    active: "Downloading",
    pending: "Queued",
    completed: "Completed",
    cancelled: "Cancelled",
    blocked: "Blocked",
    failed: "Interrupted",
  };

  const struck = (i: DownloadItem) =>
    i.status === "cancelled" || i.status === "blocked" || i.status === "failed" || (i.status === "completed" && !i.file_exists);
  const openable = (i: DownloadItem) => i.status === "completed" && i.file_exists;

  async function refresh() {
    items = await downloads.list();
  }

  onMount(() => {
    refresh();
    const unlisten = on(events.downloadsChanged, refresh);
    return () => unlisten.then((f) => f());
  });

  async function folder() {
    const configured = (store.settings as Settings).downloads.download_path;
    app.openPath(configured || (await app.downloadsDefaultDir()));
  }

  async function menu(item: DownloadItem, event: MouseEvent) {
    event.preventDefault();
    if (item.live) return;
    const entries = [];
    if (openable(item)) {
      entries.push(
        await MenuItem.new({ text: t("Open"), action: () => downloads.open(item.path) }),
        await MenuItem.new({ text: t("Show in folder"), action: () => downloads.reveal(item.path) }),
        await MenuItem.new({ text: t("Delete file"), action: () => downloads.deleteFile(item.path) }),
        await PredefinedMenuItem.new({ item: "Separator" }),
      );
    }
    if (item.path) {
      entries.push(
        await MenuItem.new({ text: t("Remove from download history"), action: () => downloads.remove(item.path) }),
      );
    }
    if (entries.length) await (await Menu.new({ items: entries })).popup();
  }
</script>

<section class="downloads">
  <header>
    <h1>{t("Downloads")}</h1>
    <button class="btn" title={t("Close")} aria-label={t("Close")} onclick={onclose}>✕</button>
  </header>

  {#if items.length === 0}
    <p class="empty muted">{t("No recent downloads")}</p>
  {:else}
    <ul>
      {#each items as item (item.key)}
        <li oncontextmenu={(e) => menu(item, e)}>
          <button
            class="name"
            class:struck={struck(item)}
            title={item.path || item.name}
            disabled={!openable(item)}
            onclick={() => downloads.open(item.path)}
          >
            {elideName(item.name)}
          </button>
          <span class="status muted">{t(STATUS[item.status])}</span>
          <span class="row-actions">
            {#if !item.live && item.file_exists}
              <button class="icon" title={t("Show in folder")} onclick={() => downloads.reveal(item.path)}>📂</button>
            {/if}
            {#if openable(item)}
              <button class="icon" title={t("Delete")} onclick={() => downloads.deleteFile(item.path)}>🗑</button>
            {/if}
          </span>
        </li>
      {/each}
    </ul>
  {/if}

  <footer>
    <button
      class="btn"
      disabled={!items.some((i) => !i.live)}
      title={t("Clear download history")}
      onclick={() => downloads.clearHistory()}>{t("Clear download history")}</button
    >
    <button class="btn" title={t("Open downloads folder")} onclick={folder}>{t("Open downloads folder")}</button>
    <button class="btn" title={t("Settings")} onclick={onsettings}>⚙ {t("Settings")}</button>
  </footer>
</section>

<svelte:window onkeydown={(e) => e.key === "Escape" && !e.defaultPrevented && onclose()} />

<style>
  .downloads {
    display: flex;
    flex-direction: column;
    height: 100%;
    max-width: 640px;
    margin: 0 auto;
    padding: 16px 20px;
    box-sizing: border-box;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h1 {
    margin: 0;
    font-size: 20px;
  }
  .empty {
    flex: 1;
  }
  ul {
    flex: 1;
    min-height: 0;
    margin: 12px 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 50px;
    padding: 0 8px;
    border-bottom: 1px solid var(--csr-mid);
  }
  li:hover {
    background: var(--csr-hover);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }
  .name:disabled {
    cursor: default;
  }
  .struck {
    text-decoration: line-through;
    opacity: 0.6;
  }
  .status {
    font-size: 12px;
  }
  .row-actions {
    display: flex;
    gap: 4px;
    visibility: hidden;
  }
  li:hover .row-actions {
    visibility: visible;
  }
  .icon {
    padding: 2px 6px;
    border: 0;
    background: none;
    cursor: pointer;
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
</style>
