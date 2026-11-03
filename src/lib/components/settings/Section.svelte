<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    description = "",
    actions,
    children,
  }: { title: string; description?: string; actions?: Snippet; children: Snippet } = $props();
</script>

<section class="section">
  <header>
    <div>
      <h2>{title}</h2>
      {#if description}<p>{description}</p>{/if}
    </div>
    {#if actions}<div class="actions">{@render actions()}</div>{/if}
  </header>
  <div class="card">{@render children()}</div>
</section>

<style>
  .section {
    margin-bottom: 24px;
  }
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  p {
    margin: 2px 0 0;
    font-size: 12px;
    opacity: 0.7;
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  .card {
    border: 1px solid var(--csr-mid);
    border-radius: 10px;
    overflow: hidden;
  }
</style>
