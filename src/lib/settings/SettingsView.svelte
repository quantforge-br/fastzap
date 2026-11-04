<script lang="ts">
  import { t } from "$lib/i18n/index.svelte";
  import { app } from "$lib/ipc";
  import { store } from "$lib/stores/settings.svelte";

  import { DEFAULT_PAGE, PAGES } from "./pages";

  let { page = $bindable(DEFAULT_PAGE), onclose }: { page?: string; onclose: () => void } = $props();

  const current = $derived(PAGES.find((p) => p.id === page) ?? PAGES[0]);
  // Avoid a double click on the gear landing on Quit (ZapZap does the same).
  let armed = $state(false);
  $effect(() => {
    const timer = setTimeout(() => (armed = true), 550);
    return () => clearTimeout(timer);
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !e.defaultPrevented && onclose()} />

<div class="settings">
  <aside>
    <header>
      <div>
        <h1>{t("Settings")}</h1>
        <p class="muted">{t("Adjust accounts, appearance, notifications and advanced options.")}</p>
      </div>
      <button class="close" title={t("Close settings")} aria-label={t("Close settings")} onclick={onclose}>
        ✕
      </button>
    </header>
    <nav>
      {#each PAGES as item (item.id)}
        <button class="nav" class:active={item.id === current.id} onclick={() => (page = item.id)}>
          {t(item.label)}
        </button>
      {/each}
    </nav>
    <footer>
      <button class="btn danger" disabled={!armed} onclick={() => app.action("quit")}>{t("Quit")}</button>
    </footer>
  </aside>
  <main>
    {#if store.settings}
      {#key current.id}
        <current.component />
      {/key}
    {/if}
  </main>
</div>

<style>
  .settings {
    position: absolute;
    inset: 0;
    display: flex;
    background: var(--bg);
  }
  aside {
    display: flex;
    flex-direction: column;
    width: 260px;
    flex: none;
    padding: 16px 12px;
    box-sizing: border-box;
    background: var(--sidebar-bg);
  }
  aside header {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin-bottom: 12px;
  }
  h1 {
    margin: 0;
    font-size: 18px;
  }
  .close {
    border: 0;
    background: none;
    color: inherit;
    font-size: 16px;
    cursor: pointer;
  }
  nav {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    overflow-y: auto;
  }
  .nav {
    padding: 8px 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: inherit;
    font: inherit;
    font-size: 14px;
    text-align: left;
    cursor: pointer;
  }
  .nav:hover,
  .nav.active {
    background: var(--csr-hover);
  }
  .nav.active {
    color: var(--accent);
    font-weight: 600;
  }
  footer {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 12px;
  }
  main {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 24px 32px;
  }
</style>
