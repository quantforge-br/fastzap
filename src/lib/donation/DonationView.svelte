<script lang="ts">
  // ZapZap's donation page. FastZap ports ZapZap, so every method supports
  // ZapZap's author.
  import { t } from "$lib/i18n/index.svelte";
  import { app } from "$lib/ipc";

  import { DONATION_METHODS } from "./methods";

  let { onclose }: { onclose: () => void } = $props();
  let failure = $state("");

  async function open(id: string) {
    failure = "";
    try {
      await app.donationOpen(id);
    } catch (e) {
      failure = (e as { message?: string })?.message ?? String(e);
    }
  }
</script>

<section class="donation">
  <header>
    <div>
      <h1>{t("Help keep ZapZap alive")}</h1>
      <p class="muted">
        {t("ZapZap is free and open source. Your contributions help keep the project active, improve the application, and develop new features.")}
      </p>
      <p class="muted">{t("FastZap is a port of ZapZap; these contributions go to ZapZap's author.")}</p>
    </div>
    <button class="close" title={t("Close donations page")} aria-label={t("Close donations page")} onclick={onclose}>✕</button>
  </header>

  <h2>{t("Choose how to support")}</h2>
  <p class="muted">
    {t("Every option opens its official page in your default browser. ZapZap never processes payment information.")}
  </p>
  <div class="cards">
    {#each DONATION_METHODS as method (method.id)}
      <article class="card">
        <div class="icon" aria-hidden="true">{method.icon}</div>
        <h3>{t(method.title)}</h3>
        <p>{t(method.description)}</p>
        <p class="muted">{t("Opens in your browser")}</p>
        <button
          class="btn primary"
          title={t("Open the official {method} page in your browser", { method: t(method.title) })}
          aria-label={t("Open {method} in your browser", { method: t(method.title) })}
          onclick={() => open(method.id)}>{t("Donate")}</button
        >
      </article>
    {/each}
  </div>
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}

  <div class="info-box impact">
    <strong>{t("Your support makes a difference")}</strong><br />
    {t("Every contribution helps cover infrastructure costs, fix problems, and keep ZapZap free for everyone.")}
  </div>
</section>

<style>
  .donation {
    height: 100%;
    padding: 20px 28px;
    overflow-y: auto;
    box-sizing: border-box;
  }
  header {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }
  h1 {
    margin: 0 0 6px;
    font-size: 22px;
  }
  h2 {
    margin: 20px 0 4px;
    font-size: 16px;
  }
  h3 {
    margin: 4px 0;
    font-size: 15px;
  }
  .close {
    align-self: flex-start;
    border: 0;
    background: none;
    color: inherit;
    font-size: 16px;
    cursor: pointer;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 12px;
    margin: 12px 0;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px;
    border: 1px solid var(--csr-mid);
    border-radius: 10px;
  }
  .card p {
    margin: 0;
    font-size: 13px;
    flex-grow: 0;
  }
  .card p:not(.muted) {
    flex-grow: 1;
  }
  .card .btn {
    margin-top: 8px;
    align-self: flex-start;
  }
  .icon {
    font-size: 20px;
    color: var(--accent);
  }
  .error {
    color: var(--danger);
  }
  .impact {
    margin-top: 16px;
  }
</style>
