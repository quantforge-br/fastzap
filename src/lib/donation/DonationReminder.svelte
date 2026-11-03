<script lang="ts">
  // ZapZap's startup support toaster (DonationController).
  import { onMount } from "svelte";

  import { t } from "$lib/i18n/index.svelte";
  import { app } from "$lib/ipc";
  import { patchSettings } from "$lib/stores/settings.svelte";

  let { onsupport, onclose }: { onsupport: () => void; onclose: () => void } = $props();
  let version = $state("");

  onMount(async () => {
    const info = await app.info();
    version = `v${info.version} - ${t(info.packaging)} - ${t(info.channel)}`;
  });

  async function hide() {
    await patchSettings({ notifications: { donation_message_hidden: true } });
    onclose();
  }
</script>

<div class="toaster" role="dialog" aria-label={t("FastZap")}>
  <button class="close" title={t("Close")} aria-label={t("Close")} onclick={onclose}>✕</button>
  <h2>{t("FastZap")}</h2>
  <p>{t("Your donation helps keep the project running, improve resources and guarantee your privacy and freedom.")}</p>
  <p class="muted">{version}</p>
  <div class="actions">
    <button class="btn primary" onclick={onsupport}>{t("Support FastZap")}</button>
    <button class="btn" onclick={hide}>{t("Don't show again")}</button>
    <button class="link" onclick={() => app.donationOpenWebsite()}>{t("Learn more about ZapZap ↗")}</button>
  </div>
</div>

<style>
  .toaster {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 40;
    width: min(360px, calc(100vw - 32px));
    padding: 14px 16px;
    border: 1px solid var(--csr-mid);
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 6px 24px rgb(0 0 0 / 25%);
    box-sizing: border-box;
  }
  h2 {
    margin: 0 0 6px;
    font-size: 16px;
  }
  p {
    margin: 0 0 6px;
    font-size: 13px;
  }
  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    border: 0;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  .link {
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }
</style>
