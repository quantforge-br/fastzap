<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount, tick } from "svelte";

  import DownloadPromptDialog from "$lib/downloads/DownloadPromptDialog.svelte";
  import PermissionDialog from "$lib/permissions/PermissionDialog.svelte";
  import DownloadsView from "$lib/downloads/DownloadsView.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import MenuBar from "$lib/components/MenuBar.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import SettingsView from "$lib/settings/SettingsView.svelte";
  import { spellPicker } from "$lib/spellcheck/request.svelte";
  import ReportDialog from "$lib/reporting/ReportDialog.svelte";
  import UpdateDialog from "$lib/update/UpdateDialog.svelte";
  import OpenChatDialog from "$lib/chat/OpenChatDialog.svelte";
  import Onboarding from "$lib/onboarding/Onboarding.svelte";
  import ShortcutsDialog from "$lib/shortcuts/ShortcutsDialog.svelte";
  import ChatView from "$lib/evolution/ChatView.svelte";
  import { closeReport, openReport, reportDialog } from "$lib/reporting/request.svelte";
  import { DEFAULT_PAGE } from "$lib/settings/pages";
  import { patchSettings, startSettings, store } from "$lib/stores/settings.svelte";

  import {
    accounts as accountsApi,
    app,
    downloads as downloadsApi,
    events,
    layout as layoutApi,
    on,
    reporting,
    type Account,
    type DownloadPrompt,
    type EvolutionEvent,
    type PermissionPrompt,
    type ReportView,
    type UpdateInfo,
    type LayoutState,
    type UiAction,
    type UnreadSnapshot,
  } from "$lib/ipc";
  import AccountEditDialog from "$lib/accounts/AccountEditDialog.svelte";
  import AccountsOverview from "$lib/accounts/AccountsOverview.svelte";
  import { addAccount } from "$lib/accounts/actions";
  import { showAccountMenu } from "$lib/accounts/contextMenu";
  import { iconSrc } from "$lib/accounts/icon";
  import { unreadLabel } from "$lib/shell/format";
  import { shortcutFor } from "$lib/shell/shortcuts";

  let accounts = $state<Account[]>([]);
  let layout = $state<LayoutState>({ active: null, grid: false, overlay: false });
  let unread = $state<UnreadSnapshot>({ per_account: {}, total: 0 });
  let panel = $state<string | null>(null);
  // The account an "open chat by number" request came from.
  let chatAccount = $state<number | null>(null);
  // A notification click asked an Evolution Go account to open a chat.
  let evolutionOpen = $state<{ account: number; chat: string } | null>(null);
  // First-run setup, decided once per launch.
  let startup = $state<"pending" | "onboarding" | "done">("pending");
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
  let permissionPrompts = $state<PermissionPrompt[]>([]);
  // A crash report from the last run, offered once at startup.
  let pendingReport = $state<ReportView | null>(null);
  let update = $state<UpdateInfo | null>(null);
  let showUpdate = $state(false);
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

  const overlay = $derived(panel !== null || confirm !== null || editTarget !== null || removing !== null || prompts.length > 0 || permissionPrompts.length > 0 || reportDialog.open || pendingReport !== null || showUpdate || startup === "onboarding");
  $effect(() => {
    const s = store.settings;
    if (!s || startup !== "pending") return;
    startup = s.onboarding.completed ? "done" : "onboarding";
  });
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
    if (action.action === "report_problem") return openReport();
    if (action.action === "spellcheck_languages") {
      spellPicker.requested = true;
      settingsPage = "language_downloads";
      return openPanel("settings");
    }
    if (action.action === "settings") settingsPage = DEFAULT_PAGE;
    if (action.action === "open_chat_by_number") chatAccount = action.account_id;
    openPanel(action.action);
  }

  // The menu accelerators are not installed while the local UI has focus.
  function onKeydown(e: KeyboardEvent) {
    if (confirm && e.key === "Escape") return answerClose(false);
    if (e.repeat || startup === "onboarding") return;
    const action = shortcutFor(e);
    if (!action) return;
    e.preventDefault();
    app.action(action).catch(() => {});
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
      on<PermissionPrompt>(events.permissionRequest, (p) => (permissionPrompts = [...permissionPrompts, p])),
      on(events.downloadSaved, onDownloadSaved),
      on<UpdateInfo>(events.updateAvailable, (info) => (update = info)),
      on<EvolutionEvent>(events.evolution, (e) => {
        if (e.type !== "open_chat") return;
        evolutionOpen = { account: e.account_id, chat: e.chat };
        openPanel(null);
      }),
      win.onResized(async () => (maximized = await win.isMaximized())),
    ];
    startSettings();
    reporting.pending().then((report) => (pendingReport = report));
    app.updateInfo().then((info) => (update = info));
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
{#if csr && appearance?.title_bar}
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
    {#if update}
      <button
        class="tool update"
        title={t("FastZap {version} is available").replace("{version}", update.latest)}
        aria-label={t("FastZap {version} is available").replace("{version}", update.latest)}
        onclick={() => (showUpdate = true)}>⬆</button
      >
    {/if}
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
      <SettingsView bind:page={settingsPage} onclose={() => openPanel(null)} />
    {:else if panel === "downloads"}
      <DownloadsView
        onclose={() => openPanel(null)}
        onsettings={() => {
          settingsPage = "language_downloads";
          openPanel("settings");
        }}
      />
    {:else if confirm}
      <!-- Shown in place of the hidden webviews. -->
    {:else if layout.grid}
      <AccountsOverview
        {accounts}
        {unread}
        columns={appearance?.grid_cols ?? 2}
        onedit={(account) => (editing = account)}
        onremove={(account) => (removing = account)}
      />
    {:else if active?.kind === "evolution_go"}
      {#key active.id}
        <ChatView
          account={active}
          openRequest={evolutionOpen?.account === active.id ? { chat: evolutionOpen.chat } : null}
        />
      {/key}
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

{#if permissionPrompts.length > 0}
  {#key permissionPrompts[0].id}
    <PermissionDialog prompt={permissionPrompts[0]} ondone={() => (permissionPrompts = permissionPrompts.slice(1))} />
  {/key}
{:else if prompts.length > 0}
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

<svelte:window onkeydown={onKeydown} />


{#if pendingReport}
  <div class="backdrop">
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="crash-title">
      <h2 id="crash-title">{t("FastZap found a problem")}</h2>
      <p>{t("FastZap closed unexpectedly during the last run.")}</p>
      <p class="muted">
        {t(
          "A sanitized report was prepared on this device. FastZap will not send it; after reviewing, you can copy it and open GitHub.",
        )}
      </p>
      <div class="actions">
        <button
          onclick={() => {
            if (pendingReport?.id) reporting.keep(pendingReport.id);
            pendingReport = null;
          }}>{t("Keep locally")}</button
        >
        <button
          class="danger"
          onclick={() => {
            if (pendingReport?.id) reporting.remove(pendingReport.id);
            pendingReport = null;
          }}>{t("Discard report")}</button
        >
        <!-- svelte-ignore a11y_autofocus -->
        <button
          class="primary"
          autofocus
          onclick={() => {
            const report = pendingReport;
            pendingReport = null;
            if (report?.id) reporting.keep(report.id);
            openReport(report);
          }}>{t("Review report")}</button
        >
      </div>
    </div>
  </div>
{/if}

{#if showUpdate && update}
  <UpdateDialog info={update} onclose={() => (showUpdate = false)} />
{/if}

{#if reportDialog.open}
  <ReportDialog review={reportDialog.review} onclose={closeReport} />
{/if}

{#if panel === "open_chat_by_number"}
  <OpenChatDialog accountId={chatAccount} onclose={() => openPanel(null)} />
{:else if panel === "show_shortcuts"}
  <ShortcutsDialog onclose={() => openPanel(null)} />
{/if}

{#if startup === "onboarding" && store.settings}
  <Onboarding settings={store.settings} onclose={() => (startup = "done")} />
{/if}

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
  .tool.update {
    color: var(--accent, #25d366);
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
  .placeholder {
    padding: 24px;
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
