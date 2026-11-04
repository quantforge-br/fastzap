<script lang="ts">
  import Icon from "./Icon.svelte";

  // A contact photo, or the grey silhouette WhatsApp shows without one.
  let { url = null, group = false, size = 49 }: { url?: string | null; group?: boolean; size?: number } = $props();
</script>

<span class="avatar" style:width="{size}px" style:height="{size}px">
  {#if url}
    <img src={url} alt="" referrerpolicy="no-referrer" />
  {:else}
    <Icon name={group ? "group" : "person"} size={Math.round(size * 0.6)} />
  {/if}
</span>

<style>
  .avatar {
    display: grid;
    flex: none;
    place-items: center;
    overflow: hidden;
    border-radius: 50%;
    background: var(--wa-avatar-bg);
    color: var(--wa-avatar-fg);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
