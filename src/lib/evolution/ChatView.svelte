<script lang="ts">
  import { onMount } from "svelte";

  import { t, uiLocale } from "$lib/i18n/index.svelte";
  import { evolution, events, on, type Account, type ChatSummary, type Connection, type EvolutionEvent, type QrCode } from "$lib/ipc";

  import Avatar from "./Avatar.svelte";
  import Conversation from "./Conversation.svelte";
  import { connectionText, jidForNumber, kindLabel, listTime, statusIcon } from "./format";
  import Icon, { type IconName } from "./Icon.svelte";
  import PairingPanel from "./PairingPanel.svelte";

  // The native chat of an Evolution Go account, shown in place of a webview.
  // Its layout follows WhatsApp Web.
  let { account, openRequest = null }: { account: Account; openRequest?: { chat: string } | null } = $props();

  type Filter = "all" | "unread" | "groups";
  const PREVIEW_ICONS: Partial<Record<ChatSummary["preview_kind"], IconName>> = {
    image: "photo",
    video: "video",
    audio: "mic",
    document: "document",
    sticker: "emoji",
    location: "location",
    contact: "person",
    poll: "poll",
  };

  let connection = $state<Connection>({ state: "connecting" });
  let chats = $state<ChatSummary[]>([]);
  let selected = $state<string | null>(null);
  let search = $state("");
  let filter = $state<Filter>("all");
  let showArchived = $state(false);
  let composingNew = $state(false);
  let menuOpen = $state(false);
  let qr = $state<QrCode | null>(null);
  let syncProgress = $state<number | null>(null);
  let versions = $state<Record<string, number>>({});
  let typing = $state<Record<string, string>>({});
  let avatars = $state<Record<string, string | null>>({});
  let newChat = $state("");
  let newChatInput = $state<HTMLInputElement>();
  const typingTimers = new Map<string, ReturnType<typeof setTimeout>>();

  const accountId = $derived(account.id);
  const online = $derived(connection.state === "online");
  const linked = $derived(connection.state === "online" || connection.state === "offline" || connection.state === "connecting");
  const query = $derived(search.trim().toLowerCase());
  const matchesFilter = (c: ChatSummary) =>
    filter === "unread" ? c.unread > 0 : filter === "groups" ? c.is_group : true;
  // A search covers archived chats too, as in WhatsApp.
  const filtered = $derived(
    query
      ? chats.filter((c) => c.name.toLowerCase().includes(query) && matchesFilter(c))
      : chats.filter((c) => c.archived === showArchived && (showArchived || matchesFilter(c))),
  );
  const archivedCount = $derived(chats.filter((c) => c.archived).length);
  const unreadChats = $derived(chats.filter((c) => c.unread > 0 && !c.archived).length);
  const current = $derived(
    selected ? (chats.find((c) => c.jid === selected) ?? placeholderChat(selected)) : null,
  );

  function placeholderChat(jid: string): ChatSummary {
    return {
      jid,
      name: `+${jid.split("@")[0]}`,
      ts: 0,
      unread: 0,
      preview: "",
      preview_kind: "text",
      preview_from_me: false,
      is_group: jid.endsWith("@g.us"),
      archived: false,
      preview_status: null,
      own: false,
    };
  }

  async function refreshChats() {
    chats = await evolution.chats(accountId).catch(() => chats);
  }

  function select(jid: string | null) {
    selected = jid;
    evolution.setOpenChat(accountId, jid).catch(() => {});
    if (jid) loadAvatar(jid);
  }

  function onEvent(e: EvolutionEvent) {
    if (e.account_id !== accountId) return;
    switch (e.type) {
      case "connection":
        connection = e.connection;
        if (e.connection.state === "online") qr = null;
        break;
      case "chats":
        refreshChats();
        break;
      case "messages":
        versions = { ...versions, [e.chat]: (versions[e.chat] ?? 0) + 1 };
        break;
      case "typing": {
        typing = { ...typing, [e.chat]: e.state === "paused" ? "" : e.state };
        clearTimeout(typingTimers.get(e.chat));
        // WhatsApp stops sending "composing" without a "paused" sometimes.
        typingTimers.set(e.chat, setTimeout(() => (typing = { ...typing, [e.chat]: "" }), 15000));
        break;
      }
      case "qr":
        qr = e.qr;
        break;
      case "sync":
        syncProgress = e.progress !== null && e.progress < 100 ? e.progress : null;
        break;
      case "open_chat":
        select(e.chat);
        break;
    }
  }

  // Once per contact.
  function loadAvatar(jid: string) {
    if (jid in avatars) return;
    avatars = { ...avatars, [jid]: null };
    evolution
      .avatar(accountId, jid)
      .then((url) => (avatars = { ...avatars, [jid]: url }))
      .catch(() => {});
  }

  // List photos load as their rows scroll into view.
  function avatar(node: HTMLElement, jid: string) {
    const observer = new IntersectionObserver((entries) => {
      if (!entries.some((e) => e.isIntersecting)) return;
      observer.disconnect();
      loadAvatar(jid);
    });
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  async function openNewChat() {
    menuOpen = false;
    composingNew = true;
    await Promise.resolve();
    newChatInput?.focus();
  }

  function startChat() {
    const jid = jidForNumber(newChat);
    if (!jid) return;
    newChat = "";
    composingNew = false;
    showArchived = false;
    select(jid);
  }

  function onWindowClick(e: MouseEvent) {
    if (menuOpen && !(e.target as Element).closest?.(".list-menu")) menuOpen = false;
  }

  $effect(() => {
    if (openRequest) select(openRequest.chat);
  });

  onMount(() => {
    const unlisten = on<EvolutionEvent>(events.evolution, onEvent);
    evolution.connection(accountId).then((c) => (connection = c));
    refreshChats();
    return () => {
      evolution.setOpenChat(accountId, null).catch(() => {});
      typingTimers.forEach(clearTimeout);
      unlisten.then((f) => f());
    };
  });
</script>

<svelte:window onclick={onWindowClick} />

{#if !linked}
  <PairingPanel {accountId} {connection} {qr} />
{:else}
  <div class="wa">
    <aside class="list">
      {#if composingNew}
        <header class="drawer">
          <button class="icon" aria-label={t("Back")} onclick={() => (composingNew = false)}><Icon name="back" /></button>
          <h1>{t("New chat")}</h1>
        </header>
        <div class="new">
          <label for="new-chat-number">{t("Phone number with country code")}</label>
          <input
            id="new-chat-number"
            bind:this={newChatInput}
            type="tel"
            placeholder="+55 11 99999-9999"
            bind:value={newChat}
            onkeydown={(e) => e.key === "Enter" && startChat()}
          />
          <button class="start" disabled={!jidForNumber(newChat)} onclick={startChat}>
            <Icon name="newChat" size={20} />{t("Start chat")}
          </button>
        </div>
      {:else}
        <header class="top">
          {#if showArchived}
            <button class="icon" aria-label={t("Back")} onclick={() => (showArchived = false)}><Icon name="back" /></button>
            <h1>{t("Archived")}</h1>
          {:else}
            <h1>{t("Chats")}</h1>
            <div class="list-menu">
              <button class="icon" aria-label={t("Menu")} aria-expanded={menuOpen} onclick={() => (menuOpen = !menuOpen)}>
                <Icon name="menu" />
              </button>
              {#if menuOpen}
                <div class="menu" role="menu">
                  <button role="menuitem" onclick={openNewChat}>{t("New chat")}</button>
                  <button role="menuitem" onclick={() => ((menuOpen = false), evolution.refreshContacts(accountId).catch(() => {}))}>
                    {t("Refresh contacts")}
                  </button>
                  <button role="menuitem" onclick={() => ((menuOpen = false), evolution.reconnect(accountId).catch(() => {}))}>
                    {t("Reconnect")}
                  </button>
                </div>
              {/if}
            </div>
            <button class="new-chat" aria-label={t("New chat")} title={t("New chat")} onclick={openNewChat}>
              <Icon name="newChat" size={20} />
            </button>
          {/if}
        </header>
        {#if !online}
          <div class="banner" role="status">
            <span>{connectionText(connection, t)}</span>
            {#if connection.state === "offline"}
              <button onclick={() => evolution.reconnect(accountId).catch(() => {})}>{t("Retry")}</button>
            {/if}
          </div>
        {/if}
        {#if syncProgress !== null}
          <div class="banner sync" role="status">{t("Syncing history… {progress}%", { progress: syncProgress })}</div>
        {/if}
        <label class="search">
          <Icon name="search" size={20} />
          <input type="text" placeholder={t("Search or start a new chat")} bind:value={search} />
          {#if search}
            <button class="clear" aria-label={t("Clear search")} onclick={() => (search = "")}><Icon name="close" size={18} /></button>
          {/if}
        </label>
        {#if !showArchived}
          <div class="chips" role="tablist">
            <button role="tab" aria-selected={filter === "all"} class:on={filter === "all"} onclick={() => (filter = "all")}>
              {t("All")}
            </button>
            <button role="tab" aria-selected={filter === "unread"} class:on={filter === "unread"} onclick={() => (filter = "unread")}>
              {t("Unread")}{#if unreadChats}<span class="chip-count">{unreadChats}</span>{/if}
            </button>
            <button role="tab" aria-selected={filter === "groups"} class:on={filter === "groups"} onclick={() => (filter = "groups")}>
              {t("Groups")}
            </button>
          </div>
        {/if}
        <ul>
          {#if !query && !showArchived && archivedCount}
            <li>
              <button class="row folder" onclick={() => (showArchived = true)}>
                <span class="folder-icon"><Icon name="archive" /></span>
                <span class="name">{t("Archived")}</span>
                <span class="folder-count">{archivedCount}</span>
              </button>
            </li>
          {/if}
          {#if showArchived && !query}
            <li class="hint">{t("These chats stay archived when new messages arrive, as on your phone.")}</li>
          {/if}
          {#each filtered as c (c.jid)}
            <li>
              <button class="row" class:active={c.jid === selected} onclick={() => select(c.jid)}>
                <span use:avatar={c.jid}><Avatar url={avatars[c.jid]} group={c.is_group} /></span>
                <span class="body">
                  <span class="line">
                    <span class="name">{c.name}{#if c.own}<span class="you">&nbsp;({t("You")})</span>{/if}</span>
                    <span class="time" class:unread={c.unread > 0}>{listTime(c.ts, uiLocale())}</span>
                  </span>
                  <span class="line">
                    <span class="preview">
                      {#if typing[c.jid]}
                        <em class="typing">{typing[c.jid] === "recording" ? t("recording audio…") : t("typing…")}</em>
                      {:else}
                        {#if c.preview_from_me && c.preview_status}
                          <span class="tick" class:read={c.preview_status === "read"} class:bad={statusIcon(c.preview_status) === "alert"}>
                            <Icon name={statusIcon(c.preview_status)} size={16} />
                          </span>
                        {/if}
                        {#if PREVIEW_ICONS[c.preview_kind]}<Icon name={PREVIEW_ICONS[c.preview_kind]!} size={16} />{/if}
                        <span class="preview-text">{c.preview || kindLabel(c.preview_kind, t)}</span>
                      {/if}
                    </span>
                    {#if c.unread > 0}<span class="badge">{c.unread}</span>{/if}
                  </span>
                </span>
              </button>
            </li>
          {:else}
            <li class="empty">
              {#if query}
                {t("No chats match the search.")}
              {:else if showArchived}
                {t("No archived chats.")}
              {:else if filter === "unread"}
                {t("No unread chats.")}
              {:else if filter === "groups"}
                {t("No groups.")}
              {:else}
                {t("Chats appear here as messages arrive or history syncs.")}
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </aside>
    {#if current}
      <Conversation
        {accountId}
        chat={current}
        avatar={avatars[current.jid] ?? null}
        {online}
        typing={typing[current.jid] ?? ""}
        version={versions[current.jid] ?? 0}
        onclose={() => select(null)}
      />
    {:else}
      <section class="idle">
        <span class="idle-icon"><Icon name="newChat" size={72} /></span>
        <h2>{t("Evolution Go")}</h2>
        <p>{t("Select a chat to start messaging.")}</p>
        <p class="small">
          {t("Archived chats follow the phone; pinning, archiving and muting are not available through Evolution Go.")}
        </p>
        <p class="small lock"><Icon name="lock" size={14} />{t("Messages go through your Evolution Go server.")}</p>
      </section>
    {/if}
  </div>
{/if}

<style>
  /* WhatsApp Web colors, shared with the conversation through inheritance. */
  .wa {
    --wa-panel: #ffffff;
    --wa-chat-bg: #f5f1eb;
    --wa-doodle: rgb(0 0 0 / 6%);
    --wa-text: #0a0a0a;
    --wa-secondary: #636261;
    --wa-icon: #3b3a39;
    --wa-hover: #f7f5f3;
    --wa-active: #f0eeeb;
    --wa-field: #f7f5f3;
    --wa-border: #e9e7e4;
    --wa-green: #1daa61;
    --wa-on-green: #ffffff;
    --wa-chip-on: #d9fdd3;
    --wa-chip-on-text: #15603e;
    --wa-bubble-in: #ffffff;
    --wa-bubble-out: #d9fdd3;
    --wa-bubble-shadow: 0 1px 0.5px rgb(11 20 26 / 13%);
    --wa-meta: #667781;
    --wa-tick-read: #53bdeb;
    --wa-pill: #ffffff;
    --wa-notice: #fff3c4;
    --wa-notice-text: #54656f;
    --wa-quote-bg: rgb(0 0 0 / 5%);
    --wa-avatar-bg: #dfe5e7;
    --wa-avatar-fg: #ffffff;
    --wa-danger: #ea0038;
    --wa-menu: #ffffff;
    --wa-menu-shadow: 0 2px 8px rgb(11 20 26 / 26%);

    display: grid;
    grid-template-columns: minmax(300px, 30%) 1fr;
    /* An auto row would grow with the messages and push the composer out. */
    grid-template-rows: minmax(0, 1fr);
    height: 100%;
    min-height: 0;
    background: var(--wa-panel);
    color: var(--wa-text);
    font-size: 14px;
  }
  @media (prefers-color-scheme: dark) {
    .wa {
      --wa-panel: #161717;
      --wa-chat-bg: #161717;
      --wa-doodle: rgb(255 255 255 / 5%);
      --wa-text: #fafafa;
      --wa-secondary: #a5a5a5;
      --wa-icon: #d1d1d1;
      --wa-hover: #242626;
      --wa-active: #2e2f2f;
      --wa-field: #2e2f2f;
      --wa-border: #2e2f2f;
      --wa-green: #21c063;
      --wa-on-green: #111b21;
      --wa-chip-on: #103529;
      --wa-chip-on-text: #d9fdd3;
      --wa-bubble-in: #242626;
      --wa-bubble-out: #144d37;
      --wa-bubble-shadow: none;
      --wa-meta: rgb(255 255 255 / 60%);
      --wa-pill: #242626;
      --wa-notice: #242626;
      --wa-notice-text: #ffd279;
      --wa-quote-bg: rgb(0 0 0 / 25%);
      --wa-avatar-bg: #3b3d3d;
      --wa-avatar-fg: #a5a5a5;
      --wa-danger: #fb5061;
      --wa-menu: #242626;
      --wa-menu-shadow: 0 2px 8px rgb(0 0 0 / 50%);
    }
  }
  .wa :global(button) {
    font: inherit;
  }
  .list {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--wa-border);
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 64px;
    padding: 0 16px 0 20px;
  }
  header.drawer {
    padding-left: 8px;
  }
  h1 {
    flex: 1;
    margin: 0;
    font-size: 22px;
    font-weight: 700;
  }
  .drawer h1,
  .top .icon + h1 {
    font-size: 16px;
    font-weight: 500;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: none;
    color: var(--wa-icon);
    cursor: pointer;
  }
  .icon:hover,
  .icon[aria-expanded="true"] {
    background: var(--wa-active);
  }
  .new-chat {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border: 0;
    border-radius: 50%;
    background: var(--wa-text);
    color: var(--wa-panel);
    cursor: pointer;
  }
  .list-menu {
    position: relative;
  }
  .menu {
    position: absolute;
    top: 44px;
    right: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    min-width: 200px;
    padding: 8px;
    border-radius: 16px;
    background: var(--wa-menu);
    box-shadow: var(--wa-menu-shadow);
  }
  .menu button {
    padding: 10px 12px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--wa-hover);
  }
  .banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 0 12px 8px;
    padding: 10px 14px;
    border-radius: 12px;
    background: var(--wa-notice);
    color: var(--wa-notice-text);
    font-size: 13px;
  }
  .banner button {
    padding: 4px 12px;
    border: 1px solid currentColor;
    border-radius: 16px;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 0 12px;
    padding: 0 12px 0 14px;
    height: 40px;
    border-radius: 20px;
    background: var(--wa-field);
    color: var(--wa-secondary);
  }
  .search input {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    outline: 0;
    background: none;
    color: var(--wa-text);
    font-size: 15px;
  }
  .search input::placeholder {
    color: var(--wa-secondary);
  }
  .clear {
    display: grid;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .chips {
    display: flex;
    gap: 8px;
    padding: 10px 12px 6px;
  }
  .chips button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border: 1px solid var(--wa-border);
    border-radius: 18px;
    background: none;
    color: var(--wa-secondary);
    font-size: 14px;
    cursor: pointer;
  }
  .chips button:hover {
    background: var(--wa-hover);
  }
  .chips button.on {
    border-color: transparent;
    background: var(--wa-chip-on);
    color: var(--wa-chip-on-text);
  }
  .chip-count {
    font-size: 12px;
    opacity: 0.8;
  }
  ul {
    flex: 1;
    margin: 0;
    padding: 0 8px 8px;
    overflow-y: auto;
    list-style: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    min-height: 72px;
    padding: 0 12px;
    border: 0;
    border-radius: 12px;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: var(--wa-hover);
  }
  .row.active {
    background: var(--wa-active);
  }
  .folder {
    min-height: 48px;
  }
  .folder-icon {
    display: grid;
    place-items: center;
    width: 49px;
    color: var(--wa-icon);
  }
  .folder .name {
    flex: 1;
    font-size: 16px;
  }
  .folder-count {
    color: var(--wa-green);
    font-size: 12px;
    font-weight: 600;
  }
  .hint,
  .empty {
    padding: 16px 20px;
    color: var(--wa-secondary);
    font-size: 13px;
    text-align: center;
  }
  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .line {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }
  .name {
    overflow: hidden;
    font-size: 16px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .you {
    color: var(--wa-secondary);
  }
  .time {
    flex: none;
    color: var(--wa-secondary);
    font-size: 12px;
  }
  .time.unread {
    color: var(--wa-green);
    font-weight: 600;
  }
  .preview {
    display: flex;
    align-items: center;
    gap: 3px;
    min-width: 0;
    color: var(--wa-secondary);
  }
  .preview-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .typing {
    color: var(--wa-green);
    font-style: normal;
  }
  .tick {
    display: flex;
  }
  .tick.read {
    color: var(--wa-tick-read);
  }
  .tick.bad {
    color: var(--wa-danger);
  }
  .badge {
    flex: none;
    min-width: 20px;
    padding: 0 6px;
    box-sizing: border-box;
    border-radius: 10px;
    background: var(--wa-green);
    color: var(--wa-on-green);
    font-size: 12px;
    font-weight: 600;
    line-height: 20px;
    text-align: center;
  }
  .new {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 8px 20px;
  }
  .new label {
    color: var(--wa-green);
    font-size: 14px;
  }
  .new input {
    padding: 10px 2px;
    border: 0;
    border-bottom: 2px solid var(--wa-green);
    border-radius: 0;
    outline: 0;
    background: none;
    color: var(--wa-text);
    font-size: 16px;
  }
  .start {
    display: flex;
    align-items: center;
    align-self: flex-start;
    gap: 8px;
    padding: 10px 18px;
    border: 0;
    border-radius: 20px;
    background: var(--wa-green);
    color: var(--wa-on-green);
    cursor: pointer;
  }
  .start:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .idle {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 32px;
    border-bottom: 6px solid var(--wa-green);
    background: var(--wa-chat-bg);
    color: var(--wa-secondary);
    text-align: center;
  }
  .idle-icon {
    color: var(--wa-avatar-bg);
  }
  .idle h2 {
    margin: 16px 0 4px;
    color: var(--wa-text);
    font-size: 28px;
    font-weight: 300;
  }
  .idle p {
    max-width: 460px;
    margin: 4px 0;
  }
  .small {
    font-size: 13px;
  }
  .lock {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 24px !important;
  }
</style>
