<script lang="ts">
  import { t } from "$lib/i18n/index.svelte";

  // ZapZap's keyboard shortcuts dialog.
  let { onclose }: { onclose: () => void } = $props();

  const whatsapp: [string, string][] = [
    ["New conversation", "Ctrl + Alt + N"],
    ["Mark as unread", "Ctrl + Alt + Shift + U"],
    ["Archive conversation", "Ctrl + Alt + Shift + E"],
    ["Pin conversation", "Ctrl + Alt + Shift + P"],
    ["Next conversation", "Ctrl + Alt + Tab"],
    ["Close conversation", "Escape"],
    ["Search in conversation", "Ctrl + Alt + Shift + F"],
    ["Previous conversation", "Ctrl + Alt + Shift + Tab"],
    ["Delete conversation", "Ctrl + Alt + Backspace"],
    ["Profile and status", "Ctrl + Alt + P"],
    ["Settings", "Ctrl + Alt + ,"],
    ["Decrease speed of selected voice message", "Shift + ,"],
    ["Increase speed of selected voice message", "Shift + ."],
    ["Emoji panel", "Ctrl + Alt + E"],
    ["Sticker panel", "Ctrl + Alt + S"],
    ["GIF panel", "Ctrl + Alt + G"],
    ["Lock the app", "Ctrl + Alt + L"],
    ["Mute", "Ctrl + Alt + Shift + M"],
    ["Search", "Ctrl + Alt + /"],
    ["New group", "Ctrl + Alt + Shift + N"],
    ["Extended search", "Alt + K"],
  ];
  const fastzap: [string, string][] = [
    ["Settings", "Ctrl + P"],
    ["Hide", "Ctrl + W"],
    ["Exit", "Ctrl + Q"],
    ["Fullscreen", "F11"],
    ["Automatic theme", "Ctrl + Alt + 1"],
    ["Light theme", "Ctrl + Alt + 2"],
    ["Dark theme", "Ctrl + Alt + 3"],
    ["Reset zoom", "Ctrl + 0"],
    ["Zoom in", "Ctrl + +"],
    ["Zoom out", "Ctrl + -"],
    ["Reload pages", "F5"],
    ["New conversation by phone number", "Ctrl + M"],
    ["New account", "Ctrl + U"],
    ["Grid View", "Ctrl + G"],
    ["Downloads", "Ctrl + J"],
    ["Open DevTools", "Ctrl + Shift + I"],
  ];
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
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Keyboard shortcuts")}>
    <header>
      <h2>{t("Keyboard shortcuts")}</h2>
      <button class="close" title={t("Close")} aria-label={t("Close")} onclick={onclose}>✕</button>
    </header>
    <div class="groups">
      {#each [["WhatsApp Web", whatsapp], ["FastZap", fastzap]] as [title, rows] (title)}
        <section>
          <h3>{title}</h3>
          <table>
            <tbody>
              {#each rows as [action, keys] (action)}
                <tr><td>{t(action as string)}</td><td><kbd>{keys}</kbd></td></tr>
              {/each}
            </tbody>
          </table>
        </section>
      {/each}
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
    display: flex;
    flex-direction: column;
    width: min(860px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    padding: 16px 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
    box-sizing: border-box;
  }
  header {
    display: flex;
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 17px;
  }
  h3 {
    margin: 12px 0 6px;
    font-size: 14px;
  }
  .close {
    border: 0;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 16px;
    overflow-y: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  td {
    padding: 3px 4px;
    border-top: 1px solid var(--csr-mid);
  }
  td:last-child {
    text-align: right;
    white-space: nowrap;
  }
  kbd {
    font-family: inherit;
    opacity: 0.8;
  }
</style>
