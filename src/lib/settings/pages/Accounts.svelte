<script lang="ts">
  import { onMount } from "svelte";

  import AccountEditDialog from "$lib/accounts/AccountEditDialog.svelte";
  import { addAccount, DEFAULT_ACCOUNT, setDoNotDisturb, setEnabled } from "$lib/accounts/actions";
  import { iconSrc } from "$lib/accounts/icon";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import Row from "$lib/components/settings/Row.svelte";
  import Section from "$lib/components/settings/Section.svelte";
  import Switch from "$lib/components/settings/Switch.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { accounts as accountsApi, events, on, type Account, type Settings } from "$lib/ipc";
  import { store } from "$lib/stores/settings.svelte";

  import Page from "../Page.svelte";

  let list = $state<Account[]>([]);
  let editing = $state<Account | null>(null);
  let removing = $state<Account | null>(null);
  let alert = $state<{ title: string; message: string } | null>(null);

  const limit = $derived((store.settings as Settings).system.max_accounts);
  const full = $derived(list.length >= limit);
  // Keep the dialog bound to the live account so a photo sync shows up.
  const editTarget = $derived(editing ? (list.find((a) => a.id === editing!.id) ?? null) : null);

  onMount(() => {
    const unlisten = on<Account[]>(events.accountsChanged, (a) => (list = a));
    accountsApi.list().then((a) => (list = a));
    return () => unlisten.then((f) => f());
  });

  async function add() {
    if (full) {
      alert = {
        title: t("Information"),
        message: t("Limit of {} users reached.\nIt is not possible to create more users!").replace("{}", String(limit)),
      };
      return;
    }
    await addAccount();
  }

  async function guard(action: Promise<unknown>, title: string, message: string) {
    try {
      await action;
    } catch {
      alert = { title, message };
    }
  }

  function toggleEnabled(account: Account, enabled: boolean) {
    guard(
      setEnabled(account.id, enabled),
      t("Could not update account"),
      t("The account state could not be changed."),
    );
  }

  async function remove() {
    const target = removing;
    removing = null;
    if (!target) return;
    await guard(
      accountsApi.remove(target.id),
      t("Could not remove account"),
      t("The account could not be removed. Please try again."),
    );
  }
</script>

<Page title={t("Accounts")} description={t("Manage your accounts and individual preferences.")}>
  <Section title={t("Account list")} description={t("Each account can have independent settings.")}>
    {#snippet actions()}
      <button class="btn primary" disabled={full} onclick={add}>+ {t("Add account")}</button>
    {/snippet}
    <p class="limit muted">
      {full
        ? t("Account limit of {limit} reached.", { limit })
        : t("{count} of {limit} accounts configured.", { count: list.length, limit })}
    </p>
  </Section>

  {#each list as account (account.id)}
    {@const name = account.name || t("Unnamed account")}
    <section class="card">
      <header>
        <img class="avatar" src={iconSrc(account.icon)} alt="" />
        <span class="name" title={name}>{name}</span>
        <button class="btn" title={t("Edit account")} onclick={() => (editing = account)}>{t("Edit")}</button>
        <button
          class="btn danger"
          disabled={account.id === DEFAULT_ACCOUNT}
          title={account.id === DEFAULT_ACCOUNT ? t("The default account cannot be removed.") : t("Remove account")}
          onclick={() => (removing = account)}>{t("Remove")}</button
        >
      </header>
      <Row
        title={t("Account status")}
        description={t("Disabled accounts remain saved, but are not loaded and do not receive notifications.")}
      >
        {#snippet control()}
          <div class="segmented" role="radiogroup" aria-label={t("Account status")}>
            <button role="radio" aria-checked={account.enabled} onclick={() => toggleEnabled(account, true)}>
              {t("Enabled")}
            </button>
            <button role="radio" aria-checked={!account.enabled} onclick={() => toggleEnabled(account, false)}>
              {t("Disabled")}
            </button>
          </div>
        {/snippet}
      </Row>
      <Row
        title={t("Do not disturb")}
        description={account.enabled
          ? t("Silences notifications for this account.")
          : t("Unavailable while the account is disabled.")}
        disabled={!account.enabled}
      >
        {#snippet control()}
          <Switch
            checked={!account.notifications}
            label={t("Do not disturb")}
            disabled={!account.enabled}
            onchange={(dnd) => setDoNotDisturb(account.id, dnd)}
          />
        {/snippet}
      </Row>
    </section>
  {/each}
</Page>

{#if editTarget}
  <AccountEditDialog account={editTarget} onclose={() => (editing = null)} />
{/if}

{#if removing}
  <ConfirmDialog
    title={t("Remove account?")}
    message={t('The account "{name}" will be removed from FastZap.', {
      name: removing.name || t("Unnamed account"),
    })}
    detail={t(
      "Its session will be closed and related local data may be permanently removed. This action cannot be undone.",
    )}
    confirmLabel={t("Remove")}
    danger
    onconfirm={remove}
    oncancel={() => (removing = null)}
  />
{/if}

{#if alert}
  <ConfirmDialog
    title={alert.title}
    message={alert.message}
    confirmLabel={t("OK")}
    onconfirm={() => (alert = null)}
    oncancel={() => (alert = null)}
  />
{/if}

<style>
  .limit {
    margin: 0;
    padding: 12px 16px;
    font-size: 13px;
  }
  .card {
    margin-bottom: 12px;
    border: 1px solid var(--csr-mid);
    border-radius: 10px;
    overflow: hidden;
  }
  .card header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--csr-mid);
  }
  .avatar {
    width: 36px;
    height: 36px;
    border-radius: 50%;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .segmented {
    display: inline-flex;
    border: 1px solid var(--csr-mid);
    border-radius: 8px;
    overflow: hidden;
  }
  .segmented button {
    padding: 4px 12px;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .segmented button[aria-checked="true"] {
    background: var(--accent);
    color: #fff;
  }
</style>
