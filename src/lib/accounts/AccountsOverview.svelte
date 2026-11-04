<script lang="ts">
  // Grid view: ZapZap's account overview (BrowserGridView). Native cards for
  // every account, WhatsApp Web and Evolution Go alike; the account webviews
  // are hidden while it is shown.
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { accounts as accountsApi, type Account, type UnreadSnapshot } from "$lib/ipc";

  import { setEnabled } from "./actions";
  import { showAccountMenu } from "./contextMenu";
  import { iconSrc } from "./icon";

  let {
    accounts,
    unread,
    columns,
    onedit,
    onremove,
  }: {
    accounts: Account[];
    unread: UnreadSnapshot;
    columns: number;
    onedit: (account: Account) => void;
    onremove: (account: Account) => void;
  } = $props();

  // A disabled account asks before it is reactivated, as in ZapZap.
  let disabled = $state<Account | null>(null);

  function open(account: Account) {
    if (account.enabled) accountsApi.activate(account.id);
    else disabled = account;
  }

  async function reactivate() {
    const target = disabled;
    disabled = null;
    if (!target) return;
    try {
      await setEnabled(target.id, true);
      await accountsApi.activate(target.id);
    } catch {
      // Refused by the backend (e.g. the account was removed meanwhile).
    }
  }
</script>

<section class="overview">
  <h1>{t("Accounts overview")}</h1>
  <p class="muted">{t("Select an account to return to its chat.")}</p>

  <div class="grid" style:--columns={Math.max(1, columns)}>
    {#each accounts as account (account.id)}
      {@const count = account.enabled ? (unread.per_account[account.id] ?? 0) : 0}
      <button
        class="card"
        class:off={!account.enabled}
        aria-label={account.name || t("Account")}
        onclick={() => open(account)}
        oncontextmenu={(e) => {
          e.preventDefault();
          showAccountMenu(account, { onedit: () => onedit(account), onremove: () => onremove(account) });
        }}
      >
        <img class="avatar" src={iconSrc(account.icon)} alt="" />
        <span class="name">{account.name || t("Unnamed account")}</span>
        {#if count > 0}<span class="unread">{count}</span>{/if}
      </button>
    {:else}
      <p class="empty muted">{t("No active accounts to display.")}</p>
    {/each}
  </div>
</section>

{#if disabled}
  <ConfirmDialog
    title={t("Account disabled")}
    message={t("This account is disabled.")}
    detail={t("You can reactivate it now or use the right-click menu to manage this account.")}
    confirmLabel={t("Activate")}
    cancelLabel={t("Not now")}
    onconfirm={reactivate}
    oncancel={() => (disabled = null)}
  />
{/if}

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: 100%;
    padding: 24px 28px 28px;
    overflow-y: auto;
    box-sizing: border-box;
  }
  h1 {
    margin: 0;
    font-size: 20px;
  }
  p {
    margin: 0 0 8px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(var(--columns), minmax(220px, 1fr));
    align-items: start;
    gap: 16px;
    padding: 16px;
    border: 1px solid var(--csr-mid);
    border-radius: 18px;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    aspect-ratio: 1 / 0.78;
    min-height: 250px;
    max-height: 360px;
    padding: 22px 20px 20px;
    border: 1px solid var(--csr-mid);
    border-radius: 16px;
    background: var(--bg);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .card:hover {
    border-color: var(--accent);
    background: var(--csr-hover);
  }
  .card:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .card.off {
    opacity: 0.6;
  }
  .avatar {
    width: clamp(72px, 34%, 112px);
    aspect-ratio: 1;
    border: 1px solid var(--csr-mid);
    border-radius: 50%;
    object-fit: cover;
  }
  .name {
    max-width: 100%;
    font-size: 15px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .unread {
    color: var(--accent);
    font-size: 12px;
    font-weight: bold;
  }
  .empty {
    grid-column: 1 / -1;
    min-height: 148px;
    margin: 0;
    display: grid;
    place-items: center;
  }
</style>
