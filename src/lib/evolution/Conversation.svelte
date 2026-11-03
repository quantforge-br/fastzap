<script lang="ts">
  import { tick } from "svelte";

  import { t, uiLocale } from "$lib/i18n/index.svelte";
  import { evolution, type ChatMessage, type ChatSummary } from "$lib/ipc";

  import { bubbleTime, kindLabel, mergeMessages, previewText, startsDay, statusMark } from "./format";

  let {
    accountId,
    chat,
    typing = "",
    version = 0,
    online,
  }: {
    accountId: number;
    chat: ChatSummary;
    /** Typing state reported by the contact ("composing", "recording"). */
    typing?: string;
    /** Bumped by the parent when this chat's messages changed. */
    version?: number;
    online: boolean;
  } = $props();

  const PAGE = 50;
  const QUICK_REACTIONS = ["👍", "❤️", "😂", "😮", "😢", "🙏"];
  const target = $derived({ accountId, chat: chat.jid });

  let messages = $state<ChatMessage[]>([]);
  let exhausted = $state(false);
  let draft = $state("");
  let replyTo = $state<ChatMessage | null>(null);
  let editing = $state<ChatMessage | null>(null);
  let menuFor = $state<string | null>(null);
  let media = $state<Record<string, string>>({});
  let failure = $state("");
  let requestedOlder = $state(false);
  let list: HTMLElement;
  let composer: HTMLTextAreaElement;
  let typingTimer: ReturnType<typeof setTimeout> | undefined;
  let composing = false;

  function errorMessage(e: unknown): string {
    return (e as { message?: string })?.message ?? String(e);
  }

  function nearBottom(): boolean {
    return !list || list.scrollHeight - list.scrollTop - list.clientHeight < 120;
  }

  async function scrollToBottom() {
    await tick();
    if (list) list.scrollTop = list.scrollHeight;
  }

  // A new chat: load its newest page and mark it read.
  $effect(() => {
    const jid = chat.jid;
    messages = [];
    exhausted = false;
    replyTo = null;
    editing = null;
    draft = "";
    requestedOlder = false;
    evolution.messages({ accountId, chat: jid }, null, PAGE).then(async (page) => {
      if (jid !== chat.jid) return;
      messages = page;
      exhausted = page.length < PAGE;
      await scrollToBottom();
    });
    evolution.markRead({ accountId, chat: jid }).catch(() => {});
    const opened = { accountId, chat: jid };
    return () => stopTyping(opened);
  });

  // Live changes: reload the newest page and merge it.
  $effect(() => {
    if (version === 0) return;
    const jid = chat.jid;
    const stick = nearBottom();
    evolution.messages({ accountId, chat: jid }, null, PAGE).then(async (page) => {
      if (jid !== chat.jid) return;
      messages = mergeMessages(messages, page);
      if (stick) await scrollToBottom();
      if (document.visibilityState === "visible" && page.some((m) => !m.from_me)) {
        evolution.markRead({ accountId, chat: jid }).catch(() => {});
      }
    });
  });

  async function loadOlder() {
    if (exhausted || messages.length === 0) return;
    const before = messages[0].seq;
    const height = list.scrollHeight;
    const page = await evolution.messages(target, before, PAGE);
    exhausted = page.length < PAGE;
    messages = mergeMessages(messages, page);
    await tick();
    list.scrollTop = list.scrollHeight - height;
  }

  // The phone sends older messages later as a history sync.
  async function requestOlder() {
    failure = "";
    try {
      requestedOlder = await evolution.loadOlder(target);
    } catch (e) {
      failure = errorMessage(e);
    }
  }

  function onScroll() {
    if (list.scrollTop < 40) loadOlder();
  }

  function stopTyping(to = target) {
    clearTimeout(typingTimer);
    if (composing) {
      composing = false;
      evolution.typing(to, false).catch(() => {});
    }
  }

  function onInput() {
    if (editing || !online) return;
    if (!composing) {
      composing = true;
      evolution.typing(target, true).catch(() => {});
    }
    clearTimeout(typingTimer);
    typingTimer = setTimeout(() => stopTyping(), 4000);
  }

  async function submit() {
    const text = draft.trim();
    if (!text) return;
    failure = "";
    stopTyping();
    try {
      if (editing) {
        await evolution.edit(target, editing.id, text);
        editing = null;
      } else {
        const quote = replyTo?.id ?? null;
        replyTo = null;
        draft = "";
        await scrollToBottom();
        await evolution.sendText(target, text, quote);
        return;
      }
      draft = "";
    } catch (e) {
      failure = errorMessage(e);
    }
  }

  async function attach() {
    failure = "";
    try {
      const quote = replyTo?.id ?? null;
      if (await evolution.sendFile(target, draft.trim(), quote)) {
        draft = "";
        replyTo = null;
      }
    } catch (e) {
      failure = errorMessage(e);
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      submit();
    } else if (e.key === "Escape" && (replyTo || editing)) {
      e.preventDefault();
      e.stopPropagation();
      replyTo = null;
      if (editing) draft = "";
      editing = null;
    }
  }

  async function act(task: () => Promise<unknown>) {
    menuFor = null;
    failure = "";
    try {
      await task();
    } catch (e) {
      failure = errorMessage(e);
    }
  }

  function startReply(m: ChatMessage) {
    menuFor = null;
    editing = null;
    replyTo = m;
    composer?.focus();
  }

  function startEdit(m: ChatMessage) {
    menuFor = null;
    replyTo = null;
    editing = m;
    draft = m.body;
    composer?.focus();
  }

  async function loadMedia(m: ChatMessage) {
    try {
      media = { ...media, [m.id]: await evolution.media(target, m.id) };
    } catch (e) {
      failure = errorMessage(e);
    }
  }

  // Images and stickers load as they are shown; other media on request.
  function autoload(node: HTMLElement, m: ChatMessage) {
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        observer.disconnect();
        if (!media[m.id]) loadMedia(m);
      }
    });
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  const canChange = (m: ChatMessage) =>
    m.from_me && !m.revoked && (m.status === "sent" || m.status === "delivered" || m.status === "read");
</script>

<section class="conversation">
  <header>
    <div class="title">
      <strong>{chat.name}</strong>
      {#if typing === "composing"}<span class="typing">{t("typing…")}</span>
      {:else if typing === "recording"}<span class="typing">{t("recording audio…")}</span>{/if}
    </div>
  </header>

  <div class="messages" bind:this={list} onscroll={onScroll}>
    {#if exhausted}
      <div class="older">
        {#if requestedOlder}
          <span class="muted">{t("Older messages were requested from your phone and will appear when it answers.")}</span>
        {:else}
          <button class="btn" disabled={!online} onclick={requestOlder}>{t("Load older messages from phone")}</button>
        {/if}
      </div>
    {/if}
    {#each messages as m, i (m.id)}
      {#if startsDay(messages[i - 1], m)}
        <div class="day">{new Date(m.ts * 1000).toLocaleDateString(uiLocale(), { dateStyle: "medium" })}</div>
      {/if}
      <div class="line" class:mine={m.from_me}>
        <div class="bubble" class:revoked={m.revoked}>
          {#if chat.is_group && !m.from_me}<span class="sender">{m.sender_name}</span>{/if}
          {#if m.quote}
            <div class="quote">
              <span class="muted">{m.quote.text || t("Quoted message")}</span>
            </div>
          {/if}
          {#if m.revoked}
            <em class="muted">{t("This message was deleted")}</em>
          {:else}
            {#if m.kind === "image" || m.kind === "sticker"}
              {#if media[m.id]}
                <img class:sticker={m.kind === "sticker"} src={media[m.id]} alt={kindLabel(m.kind, t)} />
              {:else}
                <div class="placeholder" use:autoload={m}>{kindLabel(m.kind, t)}</div>
              {/if}
            {:else if m.kind === "video" || m.kind === "audio"}
              {#if media[m.id]}
                {#if m.kind === "video"}
                  <!-- svelte-ignore a11y_media_has_caption -->
                  <video controls src={media[m.id]}></video>
                {:else}
                  <audio controls src={media[m.id]}></audio>
                {/if}
              {:else}
                <button class="btn" onclick={() => loadMedia(m)}>▶ {kindLabel(m.kind, t)}</button>
              {/if}
            {:else if m.kind === "document"}
              <button class="btn doc" onclick={() => act(() => evolution.saveMedia(target, m.id))}>
                📄 {m.file_name || kindLabel(m.kind, t)}
              </button>
            {:else if m.kind !== "text"}
              <span class="muted">{kindLabel(m.kind, t)}</span>
            {/if}
            {#if m.body && !(m.kind === "document" && m.body === m.file_name)}
              <p class="text">{m.body}</p>
            {/if}
          {/if}
          <span class="meta">
            {#if m.edited}<span>{t("edited")}</span>{/if}
            {bubbleTime(m.ts, uiLocale())}
            {#if m.from_me}<span class="status" class:read={m.status === "read"} title={m.status}>{statusMark(m.status)}</span>{/if}
          </span>
          {#if m.reactions.length}
            <span class="reactions">
              {#each m.reactions as r (r.emoji)}<span class:mine={r.mine}>{r.emoji}{r.count > 1 ? r.count : ""}</span>{/each}
            </span>
          {/if}
        </div>
        {#if !m.revoked}
          <button class="more" aria-label={t("Message actions")} onclick={() => (menuFor = menuFor === m.id ? null : m.id)}>⋯</button>
        {/if}
        {#if menuFor === m.id}
          <div class="menu" role="menu">
            <div class="emojis">
              {#each QUICK_REACTIONS as emoji (emoji)}
                <button role="menuitem" onclick={() => act(() => evolution.react(target, m.id, emoji))}>{emoji}</button>
              {/each}
              {#if m.reactions.some((r) => r.mine)}
                <button role="menuitem" title={t("Remove reaction")} onclick={() => act(() => evolution.react(target, m.id, ""))}>✕</button>
              {/if}
            </div>
            <button role="menuitem" onclick={() => startReply(m)}>{t("Reply")}</button>
            {#if m.body}<button role="menuitem" onclick={() => act(() => navigator.clipboard.writeText(m.body))}>{t("Copy")}</button>{/if}
            {#if m.kind !== "text" && m.kind !== "location" && m.kind !== "contact" && m.kind !== "unsupported"}
              <button role="menuitem" onclick={() => act(() => evolution.saveMedia(target, m.id))}>{t("Save as…")}</button>
            {/if}
            {#if canChange(m) && m.kind === "text"}<button role="menuitem" onclick={() => startEdit(m)}>{t("Edit")}</button>{/if}
            {#if canChange(m)}
              <button role="menuitem" class="danger" onclick={() => act(() => evolution.remove(target, m.id))}>
                {t("Delete for everyone")}
              </button>
            {/if}
            {#if m.from_me && (m.status === "failed" || m.status === "unknown") && m.kind === "text"}
              <button role="menuitem" onclick={() => act(() => evolution.retry(target, m.id))}>
                {m.status === "unknown" ? t("Send again (it may already have been sent)") : t("Send again")}
              </button>
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  </div>

  {#if failure}<p class="error" role="alert">{failure}</p>{/if}

  <footer>
    {#if replyTo || editing}
      <div class="context">
        <span>
          <strong>{editing ? t("Editing message") : t("Replying to")}</strong>
          {previewText((editing ?? replyTo)!.kind, (editing ?? replyTo)!.body, t)}
        </span>
        <button
          class="close"
          aria-label={t("Cancel")}
          onclick={() => {
            if (editing) draft = "";
            replyTo = null;
            editing = null;
          }}>✕</button
        >
      </div>
    {/if}
    <div class="compose">
      <button class="btn" title={t("Attach file")} aria-label={t("Attach file")} disabled={!online || !!editing} onclick={attach}>📎</button>
      <textarea
        bind:this={composer}
        rows="1"
        placeholder={online ? t("Type a message") : t("Offline: messages cannot be sent now")}
        disabled={!online}
        bind:value={draft}
        oninput={onInput}
        onkeydown={onKeydown}
      ></textarea>
      <button class="btn primary" disabled={!online || !draft.trim()} onclick={submit}>
        {editing ? t("Save") : t("Send")}
      </button>
    </div>
  </footer>
</section>

<style>
  .conversation {
    display: flex;
    flex-direction: column;
    min-width: 0;
    height: 100%;
  }
  header {
    padding: 10px 16px;
    border-bottom: 1px solid var(--csr-mid);
  }
  .title {
    display: flex;
    flex-direction: column;
  }
  .typing {
    color: var(--accent);
    font-size: 12px;
  }
  .messages {
    flex: 1;
    overflow-y: auto;
    padding: 12px 16px;
    background: var(--sidebar-bg);
  }
  .older,
  .day {
    margin: 8px 0;
    text-align: center;
  }
  .day {
    color: inherit;
    font-size: 12px;
    opacity: 0.7;
  }
  .line {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 4px;
    margin: 4px 0;
  }
  .line.mine {
    flex-direction: row-reverse;
  }
  .bubble {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-width: min(560px, 75%);
    padding: 6px 10px;
    border-radius: 8px;
    background: var(--bg);
    overflow-wrap: anywhere;
  }
  .mine .bubble {
    background: color-mix(in srgb, var(--accent) 22%, var(--bg));
  }
  .sender {
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
  }
  .quote {
    padding: 4px 8px;
    border-left: 3px solid var(--accent);
    border-radius: 4px;
    background: rgb(0 0 0 / 6%);
    font-size: 13px;
  }
  .text {
    margin: 0;
    white-space: pre-wrap;
  }
  .bubble img,
  .bubble video {
    max-width: 320px;
    max-height: 320px;
    border-radius: 6px;
  }
  .bubble img.sticker {
    max-width: 160px;
    background: none;
  }
  .placeholder {
    display: grid;
    place-items: center;
    width: 200px;
    height: 140px;
    border-radius: 6px;
    background: rgb(0 0 0 / 8%);
    font-size: 13px;
  }
  .meta {
    align-self: flex-end;
    display: flex;
    gap: 4px;
    font-size: 11px;
    opacity: 0.7;
  }
  .status.read {
    color: #53bdeb;
  }
  .reactions {
    display: flex;
    gap: 2px;
    font-size: 13px;
  }
  .reactions .mine {
    border-bottom: 2px solid var(--accent);
  }
  .more {
    border: 0;
    background: none;
    color: inherit;
    opacity: 0.4;
    cursor: pointer;
  }
  .line:hover .more {
    opacity: 1;
  }
  .menu {
    position: absolute;
    top: 24px;
    z-index: 5;
    display: flex;
    flex-direction: column;
    min-width: 200px;
    padding: 4px;
    border: 1px solid var(--csr-mid);
    border-radius: 8px;
    background: var(--bg);
    box-shadow: 0 4px 16px rgb(0 0 0 / 20%);
  }
  .line:not(.mine) .menu {
    left: 40px;
  }
  .line.mine .menu {
    right: 40px;
  }
  .menu button {
    padding: 6px 10px;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--csr-hover);
  }
  .menu .danger {
    color: var(--danger);
  }
  .emojis {
    display: flex;
  }
  .emojis button {
    padding: 4px;
    font-size: 18px;
  }
  .error {
    margin: 0;
    padding: 4px 16px;
    color: var(--danger);
    font-size: 13px;
  }
  footer {
    border-top: 1px solid var(--csr-mid);
  }
  .context {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 16px;
    font-size: 13px;
  }
  .close {
    border: 0;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .compose {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    padding: 8px 16px;
  }
  textarea {
    flex: 1;
    max-height: 160px;
    resize: none;
  }
  .doc {
    text-align: left;
  }
</style>
