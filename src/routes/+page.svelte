<script lang="ts">
  import { onMount } from "svelte";

  import {
    accounts as accountsApi,
    app,
    events,
    layout as layoutApi,
    on,
    type Account,
    type LayoutState,
    type UiAction,
    type UnreadSnapshot,
  } from "$lib/ipc";
  import { initials, isImageIcon, unreadLabel } from "$lib/shell/format";

  let accounts = $state<Account[]>([]);
  let layout = $state<LayoutState>({ active: null, grid: false, overlay: false });
  let unread = $state<UnreadSnapshot>({ per_account: {}, total: 0 });
  let panel = $state<string | null>(null);
  let content: HTMLElement;

  const visible = $derived(accounts.filter((a) => a.enabled));
  const active = $derived(visible.find((a) => a.id === layout.active) ?? null);

  // Owned UI panels cover the native webviews, which are hidden meanwhile.
  async function openPanel(name: string | null) {
    panel = name;
    await layoutApi.setOverlay(name !== null);
  }

  function reportBounds() {
    if (!content) return;
    const r = content.getBoundingClientRect();
    layoutApi.setBounds({ x: r.left, y: r.top, width: r.width, height: r.height });
  }

  function onUiAction(action: UiAction) {
    openPanel(action.action);
  }

  onMount(() => {
    const unlisten = [
      on<Account[]>(events.accountsChanged, (list) => (accounts = list)),
      on<LayoutState>(events.layoutChanged, (state) => (layout = state)),
      on<UnreadSnapshot>(events.unreadChanged, (snapshot) => (unread = snapshot)),
      on<UiAction>(events.uiAction, onUiAction),
    ];
    Promise.all([accountsApi.list(), layoutApi.get(), app.unread()]).then(([a, l, u]) => {
      accounts = a;
      layout = l;
      unread = u;
    });

    const observer = new ResizeObserver(reportBounds);
    observer.observe(content);
    window.addEventListener("resize", reportBounds);
    reportBounds();

    return () => {
      observer.disconnect();
      window.removeEventListener("resize", reportBounds);
      unlisten.forEach((p) => p.then((f) => f()));
    };
  });
</script>

<div class="shell">
  <nav class="sidebar" aria-label="Accounts">
    {#each visible as account (account.id)}
      {@const badge = unreadLabel(unread.per_account[account.id] ?? 0)}
      <button
        class="account"
        class:active={!layout.grid && account.id === layout.active}
        title={account.name}
        aria-label={account.name}
        onclick={() => accountsApi.activate(account.id)}
      >
        {#if isImageIcon(account.icon)}
          <img src={account.icon} alt="" />
        {:else}
          <span class="initials">{initials(account.name)}</span>
        {/if}
        {#if badge}<span class="badge">{badge}</span>{/if}
        {#if !account.notifications}<span class="dnd" title="Do not disturb"></span>{/if}
      </button>
    {/each}
    <div class="spacer"></div>
    <button class="tool" title="Grid view (Ctrl+G)" onclick={() => app.action("toggle_grid")}>▦</button>
    <button class="tool" title="Settings (Ctrl+P)" onclick={() => openPanel("settings")}>⚙</button>
  </nav>

  <main class="content" bind:this={content}>
    {#if panel}
      <section class="panel">
        <header>
          <h1>{panel}</h1>
          <button onclick={() => openPanel(null)}>Close</button>
        </header>
        <p>This section is not available yet.</p>
      </section>
    {:else if active?.kind === "evolution_go"}
      <section class="placeholder">
        <p>{active.name}: Evolution Go accounts are not available yet.</p>
      </section>
    {:else if visible.length === 0}
      <section class="placeholder"><p>No enabled accounts.</p></section>
    {/if}
  </main>
</div>

<style>
  :global(:root) {
    color-scheme: light dark;
    --sidebar-bg: #f0f2f5;
    --accent: #00a884;
    --fg: #111b21;
    --bg: #ffffff;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --sidebar-bg: #202c33;
      --fg: #e9edef;
      --bg: #111b21;
    }
  }
  :global(html, body) {
    margin: 0;
    height: 100%;
    overflow: hidden;
    background: var(--bg);
    color: var(--fg);
    font-family: system-ui, sans-serif;
  }
  .shell {
    display: flex;
    height: 100vh;
  }
  .sidebar {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 64px;
    padding: 10px 0;
    background: var(--sidebar-bg);
  }
  .spacer {
    flex: 1;
  }
  .account,
  .tool {
    position: relative;
    width: 44px;
    height: 44px;
    border: 2px solid transparent;
    border-radius: 50%;
    padding: 0;
    background: none;
    color: inherit;
    cursor: pointer;
    font-size: 18px;
  }
  .account.active {
    border-color: var(--accent);
  }
  .account img,
  .initials {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    object-fit: cover;
  }
  .initials {
    display: grid;
    place-items: center;
    background: var(--accent);
    color: #fff;
    font-size: 15px;
    font-weight: 600;
  }
  .badge {
    position: absolute;
    top: -4px;
    right: -6px;
    min-width: 18px;
    padding: 0 4px;
    border-radius: 9px;
    background: #e53935;
    color: #fff;
    font-size: 11px;
    line-height: 18px;
  }
  .dnd {
    position: absolute;
    bottom: 0;
    right: 0;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #8696a0;
  }
  .content {
    flex: 1;
    min-width: 0;
    position: relative;
  }
  .panel,
  .placeholder {
    padding: 24px;
  }
  .panel header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .panel h1 {
    text-transform: capitalize;
  }
</style>
