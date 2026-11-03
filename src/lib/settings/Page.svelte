<script lang="ts">
  import type { Snippet } from "svelte";

  import RestartBar from "$lib/components/settings/RestartBar.svelte";

  let {
    title,
    description = "",
    restart = false,
    children,
  }: { title: string; description?: string; restart?: boolean; children: Snippet } = $props();
</script>

<div class="page">
  <h1>{title}</h1>
  {#if description}<p class="lead">{description}</p>{/if}
  {@render children()}
  {#if restart}<RestartBar />{/if}
</div>

<style>
  .page {
    max-width: 760px;
  }
  h1 {
    margin: 0 0 4px;
    font-size: 22px;
  }
  .lead {
    margin: 0 0 20px;
    opacity: 0.7;
    font-size: 13px;
  }
</style>
