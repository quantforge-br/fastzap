<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount, tick } from "svelte";

  import DownloadPromptDialog from "$lib/downloads/DownloadPromptDialog.svelte";
  import DownloadsView from "$lib/downloads/DownloadsView.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import MenuBar from "$lib/components/MenuBar.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import SettingsView from "$lib/settings/SettingsView.svelte";
  import { DEFAULT_PAGE } from "$lib/settings/pages";
  import { patchSettings, startSettings, store } from "$lib/stores/settings.svelte";

  import {
    accounts as accountsApi,
    app,
    downloads as downloadsApi,
    events,
    layout as layoutApi,
    on,
    type Account,
    type DownloadPrompt,
    type LayoutState,
    type UiAction,
    type UnreadSnapshot,
  } from "$lib/ipc";
  import AccountEditDialog from "$lib/accounts/AccountEditDialog.svelte";
  import { addAccount } from "$lib/accounts/actions";
  import { showAccountMenu } from "$lib/accounts/contextMenu";
  import { iconSrc } from "$lib/accounts/icon";
  import { unreadLabel } from "$lib/shell/format";

  let accounts = $state<Account[]>([]);
  let layout = $state<LayoutState>({ active: null, grid: false, overlay: false });
  let unread = $state<UnreadSnapshot>({ per_account: {}, total: 0 });
  let panel = $state<string | null>(null);
  // Close confirmation (ZapZap's "Close ZapZap" question); `quit` also
  // covers Quit from the tray or Ctrl+Q.
  let confirm = $state<{ quit: boolean } | null>(null);
  let dontAskAgain = $state(false);
  let settingsPage = $state(DEFAULT_PAGE);
  let content: HTMLElement;
  const appearance = $derived(store.settings?.appearance ?? null);
  // The interface scale zooms this webview; the native webviews are placed in
  // logical pixels.
  const scale = $derived((appearance?.scale ?? 100) / 100);
  let desktop = $state("");
  let maximized = $state(false);

  const win = getCurrentWindow();
  const csr = $derived(appearance?.csr ?? false);
  // Client-side decorations: resize edges around the content while restored.
  type ResizeDirection = Parameters<typeof win.startResizeDragging>[0];
  const edges: ResizeDirection[] = [
    "North",
    "South",
    "West",
    "East",
    "NorthWest",
    "NorthEast",
    "SouthWest",
    "SouthEast",
  ];

  // The title bar and resize frame move the native webviews' area.
  $effect(() => {
    void csr;
    void maximized;
    void scale;
    void appearance?.sidebar;
    void appearance?.menubar;
    tick().then(reportBounds);
  });

  const visible = $derived(accounts.filter((a) => a.enabled));
  const active = $derived(visible.find((a) => a.id === layout.active) ?? null);

  // Owned UI panels cover the native webviews, which are hidden meanwhile.
  function openPanel(name: string | null) {
    panel = name;
  }

  let prompts = $state<DownloadPrompt[]>([]);
  let downloading = $state(false);
  let savedBadge = $state(false);
  let savedTimer: ReturnType<typeof setTimeout> | undefined;

  async function refreshDownloads() {
    downloading = (await downloadsApi.list()).some((i) => i.live);
  }

  function onDownloadSaved() {
    clearTimeout(savedTimer);
    savedBadge = true;
    savedTimer = setTimeout(() => (savedBadge = false), 900);
  }

  let editing = $state<Account | null>(null);
  let removing = $state<Account | null>(null);
  const editTarget = $derived(editing ? (accounts.find((a) => a.id === editing!.id) ?? null) : null);

  const overlay = $derived(panel !== null || confirm !== null || editTarget !== null || removing !== null || prompts.length > 0);
  $effect(() => {
    layoutApi.setOverlay(overlay);
  });

  async function removeAccount() {
    const target = removing;
    removing = null;
    if (target) await accountsApi.remove(target.id).catch(() => {});
  }

  function reportBounds() {
    if (!content) return;
    const r = content.getBoundingClientRect();
    layoutApi.setBounds({
      x: r.left * scale,
      y: r.top * scale,
      width: r.width * scale,
      height: r.height * scale,
    });
  }

  // Menu bar actions owned by the UI.
  function onMenuAction(action: string) {
    if (action === "toggle_sidebar") {
      patchSettings({ appearance: { sidebar: !(appearance?.sidebar ?? true) } });
    } else if (action === "about") {
      settingsPage = "about";
      openPanel("settings");
    } else {
      onUiAction({ action, account_id: null });
    }
  }

  function onUiAction(action: UiAction) {
    if (action.action === "confirm_close" || action.action === "confirm_quit") {
      confirm = { quit: action.action === "confirm_quit" };
      dontAskAgain = false;
      return;
    }
    // "accounts" shows the grid, so any panel covering it is closed.
    if (action.action === "accounts") return openPanel(null);
    if (action.action === "add_account") {
      // As in ZapZap: create the account right away and show it in settings.
      // At the limit the backend refuses and the page shows the limit.
      addAccount().catch(() => {});
      settingsPage = "accounts";
      return openPanel("settings");
    }
    if (action.action === "settings") settingsPage = DEFAULT_PAGE;
    openPanel(action.action);
  }

  async function answerClose(confirmed: boolean) {
    const request = confirm;
    confirm = null;
    if (confirmed && request) await app.closeConfirmed(request.quit, dontAskAgain);
  }

  onMount(() => {
    const unlisten = [
      on<Account[]>(events.accountsChanged, (list) => (accounts = list)),
      on<LayoutState>(events.layoutChanged, (state) => (layout = state)),
      on<UnreadSnapshot>(events.unreadChanged, (snapshot) => (unread = snapshot)),
      on<UiAction>(events.uiAction, onUiAction),
      on<DownloadPrompt>(events.downloadPrompt, (p) => (prompts = [...prompts, p])),
      on(events.downloadsChanged, refreshDownloads),
      on(events.downloadSaved, onDownloadSaved),
      win.onResized(async () => (maximized = await win.isMaximized())),
    ];
    startSettings();
    app.desktop().then((d) => (desktop = d));
    win.isMaximized().then((m) => (maximized = m));
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

<div class="window" class:framed={csr && !maximized}>
{#if csr && appearance}
  <TitleBar {appearance} {desktop} {maximized} />
{/if}
{#if appearance?.menubar}
  <MenuBar {accounts} {scale} sidebar={appearance.sidebar} onaction={onMenuAction} />
{/if}
<div class="shell">
  {#if appearance?.sidebar !== false}
  <nav class="sidebar" aria-label={t("Accounts")}>
    {#each visible as account (account.id)}
      {@const badge = unreadLabel(unread.per_account[account.id] ?? 0)}
      <button
        class="account"
        class:active={!layout.grid && account.id === layout.active}
        title={account.name}
        aria-label={account.name}
        onclick={() => accountsApi.activate(account.id)}
        oncontextmenu={(e) => {
          e.preventDefault();
          showAccountMenu(account, { onedit: () => (editing = account), onremove: () => (removing = account) });
        }}
      >
        <img src={iconSrc(account.icon)} alt="" />
        {#if badge}<span class="badge">{badge}</span>{/if}
        {#if !account.notifications}<span class="dnd" title="Do not disturb"></span>{/if}
      </button>
    {/each}
    <div class="spacer"></div>
    <button
      class="tool"
      class:busy={downloading}
      title={`${t("Downloads")} (Ctrl+J)`}
      aria-label={t("Downloads")}
      onclick={() => openPanel("downloads")}
      >⬇{#if savedBadge}<span class="saved">✓</span>{/if}</button
    >
    <button class="tool" title={`${t("Grid View")} (Ctrl+G)`} onclick={() => app.action("toggle_grid")}>▦</button>
    <button
      class="tool"
      title={`${t("Settings")} (Ctrl+P)`}
      onclick={() => {
        settingsPage = DEFAULT_PAGE;
        openPanel("settings");
      }}>⚙</button
    >
  </nav>
  {/if}

  <main class="content" bind:this={content}>
    {#if panel === "settings"}
      <SettingsView bind:page={settingsPage} onclose={() => openPanel(null)} ondonate={() => openPanel("donate")} />
    {:else if panel === "downloads"}
      <DownloadsView
        onclose={() => openPanel(null)}
        onsettings={() => {
          settingsPage = "language_downloads";
          openPanel("settings");
        }}
      />
    {:else if panel}
      <section class="panel">
        <header>
          <h1>{panel}</h1>
          <button onclick={() => openPanel(null)}>Close</button>
        </header>
        <p>This section is not available yet.</p>
      </section>
    {:else if confirm}
      <!-- Shown in place of the hidden webviews. -->
    {:else if active?.kind === "evolution_go"}
      <section class="placeholder">
        <p>{active.name}: Evolution Go accounts are not available yet.</p>
      </section>
    {:else if visible.length === 0}
      <section class="placeholder"><p>No enabled accounts.</p></section>
    {/if}
  </main>
</div>
{#if csr && !maximized}
  {#each edges as edge (edge)}
    <div
      class="edge {edge.toLowerCase()}"
      role="presentation"
      onmousedown={(e) => e.button === 0 && win.startResizeDragging(edge)}
    ></div>
  {/each}
{/if}
</div>

{#if confirm}
  <div class="backdrop">
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="close-title">
      <h2 id="close-title">Close FastZap</h2>
      <p>Are you sure you want to close?</p>
      <label><input type="checkbox" bind:checked={dontAskAgain} /> Don't ask again</label>
      <div class="actions">
        <button onclick={() => answerClose(false)}>No</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class="primary" autofocus onclick={() => answerClose(true)}>Yes</button>
      </div>
    </div>
  </div>
{/if}

{#if prompts.length > 0}
  {#key prompts[0].id}
    <DownloadPromptDialog prompt={prompts[0]} ondone={() => (prompts = prompts.slice(1))} />
  {/key}
{/if}

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
    onconfirm={removeAccount}
    oncancel={() => (removing = null)}
  />
{/if}

<svelte:window onkeydown={(e) => confirm && e.key === "Escape" && answerClose(false)} />

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    box-sizing: border-box;
  }
  .window.framed {
    padding: 4px;
    background: var(--sidebar-bg);
  }
  .shell {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .edge {
    position: fixed;
    z-index: 10;
  }
  .edge.north,
  .edge.south {
    left: 8px;
    right: 8px;
    height: 4px;
    cursor: ns-resize;
  }
  .edge.west,
  .edge.east {
    top: 8px;
    bottom: 8px;
    width: 4px;
    cursor: ew-resize;
  }
  .edge.north {
    top: 0;
  }
  .edge.south {
    bottom: 0;
  }
  .edge.west {
    left: 0;
  }
  .edge.east {
    right: 0;
  }
  .edge.northwest,
  .edge.northeast,
  .edge.southwest,
  .edge.southeast {
    width: 8px;
    height: 8px;
  }
  .edge.northwest {
    top: 0;
    left: 0;
    cursor: nwse-resize;
  }
  .edge.southeast {
    bottom: 0;
    right: 0;
    cursor: nwse-resize;
  }
  .edge.northeast {
    top: 0;
    right: 0;
    cursor: nesw-resize;
  }
  .edge.southwest {
    bottom: 0;
    left: 0;
    cursor: nesw-resize;
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
  .tool.busy {
    border-color: var(--accent);
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.5;
    }
  }
  .saved {
    position: absolute;
    right: 0;
    bottom: 0;
    padding: 0 4px;
    border-radius: 6px;
    background: var(--accent);
    color: #fff;
    font-size: 8px;
    font-weight: 700;
  }
  .account.active {
    border-color: var(--accent);
  }
  .account img {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    object-fit: cover;
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
  .backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 40%);
  }
  .dialog {
    min-width: 300px;
    max-width: calc(100vw - 32px);
    padding: 20px 24px;
    border-radius: 8px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
  }
  .dialog h2 {
    margin: 0 0 8px;
    font-size: 18px;
  }
  .dialog .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
  .dialog .primary {
    background: var(--accent);
    color: #fff;
    border: 0;
    border-radius: 4px;
    padding: 6px 14px;
  }
</style>
