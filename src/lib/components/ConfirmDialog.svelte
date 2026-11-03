<script lang="ts">
  import { t } from "$lib/i18n/index.svelte";

  let {
    title,
    message,
    detail = "",
    confirmLabel,
    cancelLabel = "",
    danger = false,
    onconfirm,
    oncancel,
  }: {
    title: string;
    message: string;
    detail?: string;
    confirmLabel: string;
    cancelLabel?: string;
    danger?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  } = $props();
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      oncancel();
    }
  }}
/>

<div class="backdrop">
  <div class="dialog" role="alertdialog" aria-modal="true" aria-label={title}>
    <h2>{title}</h2>
    <p>{message}</p>
    {#if detail}<p class="muted">{detail}</p>{/if}
    <div class="actions">
      <!-- svelte-ignore a11y_autofocus -->
      <button class="btn" autofocus onclick={oncancel}>{cancelLabel || t("Cancel")}</button>
      <button class="btn" class:danger class:primary={!danger} onclick={onconfirm}>{confirmLabel}</button>
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
    width: min(440px, calc(100vw - 32px));
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
    margin: 0 0 8px;
    white-space: pre-line;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
</style>
