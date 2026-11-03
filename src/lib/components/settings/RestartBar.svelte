<script lang="ts">
  import { t } from "$lib/i18n/index.svelte";
  import { app } from "$lib/ipc";
</script>

<div class="bar" role="status">
  <span>{t("A restart is required to apply the changes.")}</span>
  <button class="btn primary" onclick={() => app.action("restart")}>{t("Restart now")}</button>
</div>

<style>
  .bar {
    position: sticky;
    bottom: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 12px;
    padding: 10px 16px;
    border-radius: 10px;
    background: var(--sidebar-bg);
    border: 1px solid var(--accent);
  }
</style>
