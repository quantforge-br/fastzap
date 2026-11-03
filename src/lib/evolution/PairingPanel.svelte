<script lang="ts">
  import { onMount } from "svelte";

  import { t } from "$lib/i18n/index.svelte";
  import { evolution, type Connection, type QrCode } from "$lib/ipc";

  import { connectionText } from "./format";

  // Link WhatsApp to the instance: QR code, phone pairing code or passkey.
  let {
    accountId,
    connection,
    qr = null,
  }: { accountId: number; connection: Connection; qr?: QrCode | null } = $props();

  let fetched = $state<QrCode | null>(null);
  let phone = $state("");
  let pairingCode = $state("");
  let busy = $state(false);
  let failure = $state("");

  // A QR pushed over the WebSocket replaces the fetched one.
  const shown = $derived(qr ?? fetched);
  const unlinked = $derived(connection.state === "logged_out");

  function errorMessage(e: unknown): string {
    return (e as { message?: string })?.message ?? String(e);
  }

  async function run(task: () => Promise<void>) {
    busy = true;
    failure = "";
    try {
      await task();
    } catch (e) {
      failure = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  const loadQr = () =>
    run(async () => {
      fetched = await evolution.qr(accountId);
    });

  const pair = () =>
    run(async () => {
      pairingCode = await evolution.pairPhone(accountId, phone);
    });

  // After an unlink the instance stays down until the user asks to link again.
  const relink = () =>
    run(async () => {
      await evolution.reconnect(accountId);
      fetched = await evolution.qr(accountId);
    });

  onMount(() => {
    if (connection.state === "pairing") loadQr();
  });
</script>

<section class="pairing">
  <h2>{t("Link WhatsApp")}</h2>
  <p class="muted">{connectionText(connection, t)}</p>

  {#if connection.state === "stopped" || connection.state === "banned"}
    <p class="muted">{t("Check this account's Evolution Go connection in Settings → Accounts.")}</p>
  {:else if unlinked}
    <button class="btn primary" disabled={busy} onclick={relink}>{t("Link again")}</button>
  {:else if shown?.passkey_stage}
    <div class="info-box">
      <strong>{t("WhatsApp asked for a passkey")}</strong><br />
      {t("Open WhatsApp Web in your browser with the Evolution Go passkey helper to confirm the link.")}
      {#if shown.passkey_code}<p class="code">{shown.passkey_code}</p>{/if}
    </div>
    <button class="btn primary" disabled={busy} onclick={() => run(() => evolution.openPasskey(accountId))}>
      {t("Open in browser")}
    </button>
  {:else}
    <ol class="steps">
      <li>{t("Open WhatsApp on your phone.")}</li>
      <li>{t("Go to Settings → Linked devices → Link a device.")}</li>
      <li>{t("Scan the code below.")}</li>
    </ol>
    <div class="qr">
      {#if shown?.qrcode.startsWith("data:image/")}
        <img src={shown.qrcode} alt={t("QR code")} />
      {:else}
        <span class="muted">{busy ? t("Loading…") : t("No QR code yet.")}</span>
      {/if}
    </div>
    <button class="btn" disabled={busy} onclick={loadQr}>{t("Refresh QR code")}</button>

    <details>
      <summary>{t("Link with phone number instead")}</summary>
      <div class="phone">
        <input type="tel" bind:value={phone} placeholder={t("Number with country code")} />
        <button class="btn" disabled={busy || phone.replace(/\D/g, "").length < 7} onclick={pair}>
          {t("Get code")}
        </button>
      </div>
      {#if pairingCode}
        <p class="code" aria-label={t("Pairing code")}>{pairingCode}</p>
        <p class="muted">{t("On your phone, choose “Link with phone number instead” and enter this code.")}</p>
      {/if}
    </details>
  {/if}

  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
</section>

<style>
  .pairing {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    max-width: 420px;
    margin: 32px auto;
    padding: 0 16px;
    text-align: center;
  }
  h2 {
    margin: 0;
  }
  p {
    margin: 0;
  }
  .steps {
    margin: 0;
    padding-left: 20px;
    text-align: left;
  }
  .qr {
    display: grid;
    place-items: center;
    width: 264px;
    height: 264px;
    border-radius: 8px;
    background: #fff;
  }
  .qr img {
    width: 256px;
    height: 256px;
    image-rendering: pixelated;
  }
  .phone {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }
  .code {
    margin: 8px 0;
    font-family: monospace;
    font-size: 24px;
    letter-spacing: 3px;
  }
  .error {
    color: var(--danger);
  }
</style>
