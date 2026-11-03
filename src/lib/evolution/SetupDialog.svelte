<script lang="ts">
  import { onMount } from "svelte";

  import { newIconSvg } from "$lib/accounts/icon";
  import { t } from "$lib/i18n/index.svelte";
  import { evolution, type Account, type InstanceSummary } from "$lib/ipc";

  // Create an Evolution Go account, or point an existing one at another
  // server or instance. The admin key goes straight to the keyring.
  let { account = null, onclose }: { account?: Account | null; onclose: () => void } = $props();

  const DEFAULT_SERVER = "https://evolutiongo.diogosrodrigues.com.br";

  let name = $state("");
  let baseUrl = $state(DEFAULT_SERVER);
  let adminKey = $state("");
  let hasSavedKey = $state(false);
  let instances = $state<InstanceSummary[] | null>(null);
  let mode = $state<"existing" | "create">("existing");
  let instanceId = $state("");
  let newInstance = $state("fastzap");
  let busy = $state(false);
  let failure = $state("");

  onMount(async () => {
    if (!account) return;
    name = account.name;
    const info = await evolution.info(account.id).catch(() => null);
    if (info) {
      baseUrl = info.base_url;
      instanceId = info.instance_id;
      hasSavedKey = info.has_admin_key;
    }
  });

  function errorMessage(e: unknown): string {
    return (e as { message?: string })?.message ?? String(e);
  }

  async function loadInstances() {
    failure = "";
    busy = true;
    try {
      // A saved key never comes back to the UI, so listing needs it typed.
      instances = await evolution.instances(baseUrl, adminKey);
      if (!instances.some((i) => i.id === instanceId)) instanceId = instances[0]?.id ?? "";
      if (instances.length === 0) mode = "create";
    } catch (e) {
      instances = null;
      failure = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  const ready = $derived(
    !busy &&
      baseUrl.trim() !== "" &&
      (adminKey.trim() !== "" || hasSavedKey) &&
      (mode === "existing" ? instanceId !== "" : newInstance.trim() !== ""),
  );

  async function save() {
    if (!ready) return;
    busy = true;
    failure = "";
    try {
      await evolution.setup({
        account_id: account?.id ?? null,
        name: name.trim(),
        icon: account ? null : newIconSvg(),
        base_url: baseUrl.trim(),
        admin_key: adminKey,
        instance: mode === "existing" ? { mode, id: instanceId } : { mode, name: newInstance.trim() },
      });
      adminKey = "";
      onclose();
    } catch (e) {
      failure = errorMessage(e);
    } finally {
      busy = false;
    }
  }
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
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Evolution Go account")}>
    <header>
      <div>
        <h2>{account ? t("Evolution Go connection") : t("Add Evolution Go account")}</h2>
        <p class="muted">{t("Use WhatsApp through your own Evolution Go server instead of WhatsApp Web.")}</p>
      </div>
      <button class="close" title={t("Close")} aria-label={t("Close")} onclick={onclose}>✕</button>
    </header>

    <label>
      {t("Account name")}
      <input type="text" bind:value={name} placeholder={t("Optional")} />
    </label>
    <label>
      {t("Server address")}
      <input type="url" bind:value={baseUrl} spellcheck="false" onchange={() => (instances = null)} />
    </label>
    <label>
      {t("Admin key")}
      <input
        type="password"
        autocomplete="off"
        bind:value={adminKey}
        placeholder={hasSavedKey ? t("Saved in the system keyring") : ""}
        onchange={() => (instances = null)}
      />
    </label>
    <p class="hint muted">
      {t("The key is kept in the system keyring and gives full access to the server. Use it only on your own computer.")}
    </p>

    <div class="row">
      <button
        class="btn"
        disabled={busy || !adminKey.trim()}
        title={hasSavedKey ? t("Type the admin key again to list the server's instances.") : ""}
        onclick={loadInstances}
      >
        {t("Load instances")}
      </button>
      {#if account && instanceId && instances === null}
        <span class="muted">{t("Current instance: {id}", { id: instanceId })}</span>
      {/if}
    </div>

    {#if instances !== null}
      <fieldset>
        <legend>{t("Instance")}</legend>
        <label class="choice">
          <input type="radio" bind:group={mode} value="existing" disabled={instances.length === 0} />
          {t("Use an existing instance")}
        </label>
        {#if mode === "existing"}
          <select bind:value={instanceId} aria-label={t("Instance")}>
            {#each instances as instance (instance.id)}
              <option value={instance.id}>
                {instance.name}{instance.jid ? ` — ${instance.jid.split("@")[0].split(":")[0]}` : ""}{instance.connected
                  ? ""
                  : ` (${t("disconnected")})`}
              </option>
            {/each}
          </select>
        {/if}
        <label class="choice">
          <input type="radio" bind:group={mode} value="create" />
          {t("Create a new instance on the server")}
        </label>
        {#if mode === "create"}
          <input type="text" bind:value={newInstance} aria-label={t("New instance name")} />
        {/if}
      </fieldset>
      <div class="info-box warning">
        {t("Connecting turns on the instance's WebSocket; its webhook and other integrations are kept.")}
      </div>
    {/if}

    {#if failure}<p class="error" role="alert">{failure}</p>{/if}

    <footer>
      <button class="btn" onclick={onclose}>{t("Cancel")}</button>
      <button class="btn primary" disabled={!ready || (instances === null && !account)} onclick={save}>
        {account ? t("Save") : t("Add account")}
      </button>
    </footer>
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
    gap: 10px;
    width: min(540px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow: auto;
    padding: 16px 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
    box-sizing: border-box;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
  }
  h2 {
    margin: 0;
    font-size: 17px;
  }
  p {
    margin: 0;
  }
  .close {
    border: 0;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  label.choice {
    flex-direction: row;
    align-items: center;
    gap: 6px;
  }
  fieldset {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    border: 1px solid var(--csr-mid);
    border-radius: 8px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .hint {
    font-size: 12px;
  }
  .error {
    color: var(--danger);
    font-size: 13px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
