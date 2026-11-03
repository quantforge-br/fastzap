<script lang="ts">
  // Quick access panel (ZapZap's FloatingMonitoringPanel), shown in its own
  // window by src-tauri/src/app/panel.rs.
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  import {
    accounts as accountsApi,
    app,
    events,
    layout as layoutApi,
    on,
    settings as settingsApi,
    type Account,
    type LayoutState,
    type Settings,
    type UnreadSnapshot,
  } from "$lib/ipc";
  import { iconSrc } from "$lib/accounts/icon";
  import { unreadLabel } from "$lib/shell/format";

  let accounts = $state<Account[]>([]);
  let active = $state<number | null>(null);
  let unread = $state<UnreadSnapshot>({ per_account: {}, total: 0 });
  let settings = $state<Settings | null>(null);

  const muted = $derived(settings?.system.audio_muted ?? false);
  const locked = $derived(settings?.appearance.floating_panel.lock_position ?? false);

  async function activate(account: Account) {
    if (!account.enabled) return;
    await accountsApi.activate(account.id);
    await app.action("show");
  }

  function unlock() {
    settingsApi.patch({ appearance: { floating_panel: { lock_position: false } } });
  }

  onMount(() => {
    const unlisten = [
      on<Account[]>(events.accountsChanged, (list) => (accounts = list)),
      on<LayoutState>(events.layoutChanged, (state) => (active = state.active)),
      on<UnreadSnapshot>(events.unreadChanged, (snapshot) => (unread = snapshot)),
      on<Settings>(events.settingsChanged, (s) => (settings = s)),
    ];
    Promise.all([accountsApi.list(), layoutApi.get(), app.unread(), settingsApi.get()]).then(
      ([a, l, u, s]) => {
        accounts = a;
        active = l.active;
        unread = u;
        settings = s;
      },
    );
    return () => unlisten.forEach((p) => p.then((f) => f()));
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && getCurrentWindow().close()} />

<div class="surface" aria-label="Monitoring panel">
  <header>
    <h1>Quick access</h1>
    {#if locked}
      <button class="icon" title="Unlock position" aria-label="Unlock position" onclick={unlock}>
        📌
      </button>
    {/if}
  </header>

  {#if accounts.length === 0}
    <p class="empty">No account activity to show yet.</p>
  {:else}
    <ul class="accounts">
      {#each accounts as account, index (account.id)}
        {@const name = account.name || `Account ${index + 1}`}
        {@const count = account.enabled ? (unread.per_account[account.id] ?? 0) : 0}
        <li>
          <button
            class="row"
            class:active={account.id === active}
            disabled={!account.enabled}
            aria-label={name}
            onclick={() => activate(account)}
          >
            <img class="avatar" src={iconSrc(account.icon)} alt="" />
            <span class="name">{name}</span>
            {#if count > 0}<span class="badge">{unreadLabel(count)}</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <button class="row" onclick={() => app.action("add_account")}>
    <span class="avatar initials">+</span>
    <span class="name">New account</span>
  </button>

  <hr />

  <div class="footer">
    <button class="action" onclick={() => app.action("toggle_mute")}>
      {muted ? "🔇 Unmute" : "🔊 Mute"}
    </button>
    <button class="action" onclick={() => app.action("downloads")}>⬇ Downloads</button>
  </div>
  <button class="action" onclick={() => app.action("settings")}>⚙ Settings</button>
</div>

<style>
  .surface {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 100%;
    box-sizing: border-box;
    padding: 14px 16px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  h1 {
    flex: 1;
    margin: 0;
    font-size: 15px;
  }
  .icon {
    width: 28px;
    height: 28px;
    border: 0;
    background: none;
    cursor: pointer;
  }
  .empty {
    margin: 0;
    font-size: 13px;
    opacity: 0.7;
  }
  .accounts {
    flex: 1;
    min-height: 0;
    max-height: 360px;
    overflow-y: auto;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .accounts li + li {
    margin-top: 2px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: 10px;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover:not(:disabled),
  .row.active {
    background: var(--csr-hover);
  }
  .row.active {
    border-color: var(--accent);
  }
  .row:disabled {
    cursor: default;
    opacity: 0.5;
  }
  .avatar {
    width: 28px;
    height: 28px;
    flex: none;
    border-radius: 50%;
    object-fit: cover;
  }
  .initials {
    display: grid;
    place-items: center;
    background: var(--accent);
    color: #fff;
    font-size: 12px;
    font-weight: 600;
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    min-width: 18px;
    padding: 0 6px;
    border-radius: 9px;
    background: var(--accent);
    color: #fff;
    font-size: 11px;
    line-height: 18px;
    text-align: center;
  }
  hr {
    width: 100%;
    margin: 0;
    border: 0;
    border-top: 1px solid var(--csr-mid);
  }
  .footer {
    display: flex;
    gap: 6px;
  }
  .footer .action {
    flex: 1;
  }
  .action {
    padding: 6px 10px;
    border: 1px solid var(--csr-mid);
    border-radius: 8px;
    background: var(--bg);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .action:hover {
    background: var(--csr-hover);
    border-color: var(--accent);
  }
</style>
