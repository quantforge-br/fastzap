<script lang="ts">
  import { onMount } from "svelte";

  import { t, uiLocale } from "$lib/i18n/index.svelte";
  import { evolution, events, on, type Account, type ChatSummary, type Connection, type EvolutionEvent, type QrCode } from "$lib/ipc";

  import Conversation from "./Conversation.svelte";
  import { connectionText, jidForNumber, listTime, previewText } from "./format";
  import PairingPanel from "./PairingPanel.svelte";

  // The native chat of an Evolution Go account, shown in place of a webview.
  let { account, openRequest = null }: { account: Account; openRequest?: { chat: string } | null } = $props();

  let connection = $state<Connection>({ state: "connecting" });
  let chats = $state<ChatSummary[]>([]);
  let selected = $state<string | null>(null);
  let search = $state("");
  let showArchived = $state(false);
  let qr = $state<QrCode | null>(null);
  let syncProgress = $state<number | null>(null);
  let versions = $state<Record<string, number>>({});
  let typing = $state<Record<string, string>>({});
  let avatars = $state<Record<string, string | null>>({});
  let newChat = $state("");
  const typingTimers = new Map<string, ReturnType<typeof setTimeout>>();

  const accountId = $derived(account.id);
  const online = $derived(connection.state === "online");
  const linked = $derived(connection.state === "online" || connection.state === "offline" || connection.state === "connecting");
  const query = $derived(search.trim().toLowerCase());
  // A search covers archived chats too, as in WhatsApp.
  const filtered = $derived(
    query ? chats.filter((c) => c.name.toLowerCase().includes(query)) : chats.filter((c) => c.archived === showArchived),
  );
  const archivedCount = $derived(chats.filter((c) => c.archived).length);
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
    };
  }

  async function refreshChats() {
    chats = await evolution.chats(accountId).catch(() => chats);
  }

  function select(jid: string | null) {
    selected = jid;
    evolution.setOpenChat(accountId, jid).catch(() => {});
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

  // Loaded lazily, once per contact.
  function avatar(node: HTMLElement, jid: string) {
    const observer = new IntersectionObserver((entries) => {
      if (!entries.some((e) => e.isIntersecting) || jid in avatars) return;
      observer.disconnect();
      avatars = { ...avatars, [jid]: null };
      evolution
        .avatar(accountId, jid)
        .then((url) => (avatars = { ...avatars, [jid]: url }))
        .catch(() => {});
    });
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  function startChat() {
    const jid = jidForNumber(newChat);
    if (!jid) return;
    newChat = "";
    select(jid);
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

{#if !linked}
  <PairingPanel {accountId} {connection} {qr} />
{:else}
  <div class="chat">
    <aside class="list">
      <div class="status" class:warn={!online}>
        <span>{connectionText(connection, t)}</span>
        {#if connection.state === "offline"}
          <button class="btn" onclick={() => evolution.reconnect(accountId).catch(() => {})}>{t("Retry")}</button>
        {/if}
      </div>
      {#if syncProgress !== null}
        <div class="sync">{t("Syncing history… {progress}%", { progress: syncProgress })}</div>
      {/if}
      <input class="search" type="text" placeholder={t("Search chats")} bind:value={search} />
      <div class="new">
        <input
          type="tel"
          placeholder={t("New chat: number with country code")}
          bind:value={newChat}
          onkeydown={(e) => e.key === "Enter" && startChat()}
        />
        <button class="btn" disabled={!jidForNumber(newChat)} onclick={startChat}>{t("Open")}</button>
      </div>
      <ul>
        {#if !query && showArchived}
          <li>
            <button class="item folder" onclick={() => (showArchived = false)}>
              <span class="avatar">←</span>
              <span class="name">{t("Archived")}</span>
            </button>
          </li>
        {:else if !query && archivedCount}
          <li>
            <button class="item folder" onclick={() => (showArchived = true)}>
              <span class="avatar">🗄</span>
              <span class="name">{t("Archived")}</span>
              <span class="count muted">{archivedCount}</span>
            </button>
          </li>
        {/if}
        {#each filtered as c (c.jid)}
          <li>
            <button class="item" class:active={c.jid === selected} onclick={() => select(c.jid)}>
              <span class="avatar" use:avatar={c.jid}>
                {#if avatars[c.jid]}<img src={avatars[c.jid]} alt="" referrerpolicy="no-referrer" />{:else}{c.is_group ? "👥" : "👤"}{/if}
              </span>
              <span class="body">
                <span class="top">
                  <span class="name">{c.name}</span>
                  <span class="time" class:unread={c.unread > 0}>{listTime(c.ts, uiLocale())}</span>
                </span>
                <span class="bottom">
                  <span class="preview">
                    {#if typing[c.jid]}<em>{t("typing…")}</em>{:else}{c.preview_from_me ? `${t("You")}: ` : ""}{previewText(c.preview_kind, c.preview, t)}{/if}
                  </span>
                  {#if c.unread > 0}<span class="badge">{c.unread}</span>{/if}
                </span>
              </span>
            </button>
          </li>
        {:else}
          <li class="empty muted">
            {#if query}
              {t("No chats match the search.")}
            {:else if showArchived}
              {t("No archived chats.")}
            {:else}
              {t("Chats appear here as messages arrive or history syncs.")}
            {/if}
          </li>
        {/each}
      </ul>
    </aside>
    {#if current}
      <Conversation
        {accountId}
        chat={current}
        {online}
        typing={typing[current.jid] ?? ""}
        version={versions[current.jid] ?? 0}
      />
    {:else}
      <section class="idle muted">
        <p>{t("Select a chat to start messaging.")}</p>
        <p class="small">{t("Archived chats follow the phone; pinning, archiving and muting are not available through Evolution Go.")}</p>
      </section>
    {/if}
  </div>
{/if}

<style>
  .chat {
    display: grid;
    grid-template-columns: minmax(260px, 340px) 1fr;
    /* An auto row would grow with the messages and push the composer out. */
    grid-template-rows: minmax(0, 1fr);
    height: 100%;
    min-height: 0;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
    border-right: 1px solid var(--csr-mid);
  }
  .status {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 8px 12px;
    font-size: 12px;
  }
  .status.warn {
    background: color-mix(in srgb, var(--danger) 14%, var(--bg));
  }
  .sync {
    padding: 0 12px;
    font-size: 12px;
  }
  .search,
  .new {
    margin: 0 12px;
  }
  .new {
    display: flex;
    gap: 6px;
  }
  .new input {
    flex: 1;
    min-width: 0;
  }
  ul {
    flex: 1;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }
  .item {
    display: flex;
    gap: 10px;
    width: 100%;
    padding: 8px 12px;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .item:hover {
    background: var(--csr-hover);
  }
  .folder {
    align-items: center;
  }
  .folder .name {
    flex: 1;
  }
  .count {
    font-size: 12px;
  }
  .item.active {
    background: var(--sidebar-bg);
  }
  .avatar {
    display: grid;
    flex: none;
    place-items: center;
    width: 42px;
    height: 42px;
    overflow: hidden;
    border-radius: 50%;
    background: var(--csr-mid);
  }
  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .top,
  .bottom {
    display: flex;
    justify-content: space-between;
    gap: 6px;
  }
  .name,
  .preview {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    font-weight: 600;
  }
  .preview,
  .time {
    font-size: 12px;
    opacity: 0.75;
  }
  .time.unread {
    color: var(--accent);
    opacity: 1;
  }
  .badge {
    min-width: 18px;
    padding: 0 5px;
    border-radius: 9px;
    background: var(--accent);
    color: #fff;
    font-size: 11px;
    line-height: 18px;
    text-align: center;
  }
  .empty {
    padding: 16px;
    font-size: 13px;
  }
  .idle {
    display: grid;
    place-content: center;
    text-align: center;
  }
  .small {
    font-size: 12px;
  }
</style>
