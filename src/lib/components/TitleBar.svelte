<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import type { AppearanceSettings } from "$lib/ipc";
  import { CSR_BUTTON_THEMES, resolveCsrButtonTheme } from "$lib/shell/csr";

  let {
    appearance,
    desktop,
    maximized,
  }: { appearance: AppearanceSettings; desktop: string; maximized: boolean } = $props();

  const win = getCurrentWindow();
  const theme = $derived(resolveCsrButtonTheme(appearance.csr_button_theme, desktop));
  const buttons = $derived(CSR_BUTTON_THEMES[theme]);
  const left = $derived(appearance.csr_buttons_direction === "left");
</script>

<!-- Dragging and double-click maximize come from data-tauri-drag-region. -->
<header
  class="titlebar"
  class:left
  data-tauri-drag-region
  style:--csr-font={`${buttons.fontSize}px`}
  style:--csr-width={`${buttons.width}px`}
  style:--csr-height={`${buttons.height}px`}
  style:--csr-radius={`${buttons.radius}px`}
>
  <span class="title" data-tauri-drag-region>FastZap</span>
  <div class="buttons">
    {#if appearance.csr_show_minimize_button}
      <button title="Minimize" onclick={() => win.minimize()}>{buttons.minimize}</button>
    {/if}
    {#if appearance.csr_show_maximize_button}
      <button title={maximized ? "Restore" : "Maximize"} onclick={() => win.toggleMaximize()}>
        {buttons.maximize}
      </button>
    {/if}
    <button class="close" class:adwaita={theme === "adwaita"} title="Close" onclick={() => win.close()}>
      {buttons.close}
    </button>
  </div>
</header>

<style>
  .titlebar {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 40px;
    flex: none;
    padding: 0 8px 0 12px;
    background: var(--sidebar-bg);
    user-select: none;
  }
  .titlebar.left {
    flex-direction: row-reverse;
    padding: 0 12px 0 8px;
  }
  .title {
    flex: 1;
    font-size: 12px;
  }
  .titlebar.left .title {
    padding-left: 8px;
  }
  .buttons {
    display: flex;
    gap: 6px;
  }
  /* Buttons keep the order minimize, maximize, close on both sides. */
  button {
    width: var(--csr-width);
    height: var(--csr-height);
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--csr-radius);
    background: var(--sidebar-bg);
    color: var(--fg);
    font-size: var(--csr-font);
    font-weight: 500;
    cursor: pointer;
  }
  button:hover {
    background: var(--csr-hover);
    border-color: var(--csr-mid);
  }
  button:active {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .close {
    background: var(--danger);
    color: #fff;
  }
  .close:hover {
    background: var(--danger);
  }
  .close.adwaita {
    background: var(--csr-hover);
    color: var(--fg);
  }
  .close.adwaita:hover {
    background: var(--csr-mid);
  }
</style>
