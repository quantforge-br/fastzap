<script lang="ts">
  import type { Snippet } from "svelte";

  // One settings row: title and description on the left, control on the right.
  // `child` indents rows that depend on a parent switch.
  let {
    title,
    description = "",
    disabled = false,
    child = false,
    control,
    children,
  }: {
    title: string;
    description?: string;
    disabled?: boolean;
    child?: boolean;
    control?: Snippet;
    children?: Snippet;
  } = $props();
</script>

<div class="row" class:disabled class:child>
  <div class="text">
    <div class="title">{title}</div>
    {#if description}<div class="description">{description}</div>{/if}
    {#if children}{@render children()}{/if}
  </div>
  {#if control}<div class="control">{@render control()}</div>{/if}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 16px;
  }
  .row + :global(.row) {
    border-top: 1px solid var(--csr-mid);
  }
  .child {
    padding-left: 36px;
  }
  .disabled {
    opacity: 0.5;
    pointer-events: none;
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-size: 14px;
  }
  .description {
    margin-top: 2px;
    font-size: 12px;
    opacity: 0.7;
  }
  .control {
    display: flex;
    flex: none;
    gap: 6px;
    align-items: center;
  }
</style>
