<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { tick } from "svelte";

  import { t, uiLocale } from "$lib/i18n/index.svelte";
  import { evolution, type ChatMessage, type ChatSummary } from "$lib/ipc";

  import Avatar from "./Avatar.svelte";
  import {
    bubbleTime,
    dayLabel,
    kindLabel,
    linkParts,
    mergeMessages,
    previewText,
    senderColor,
    startsDay,
    startsGroup,
    statusIcon,
  } from "./format";
  import Icon from "./Icon.svelte";

  let {
    accountId,
    chat,
    avatar = null,
    typing = "",
    version = 0,
    online,
    onclose,
  }: {
    accountId: number;
    chat: ChatSummary;
    avatar?: string | null;
    /** Typing state reported by the contact ("composing", "recording"). */
    typing?: string;
    /** Bumped by the parent when this chat's messages changed. */
    version?: number;
    online: boolean;
    onclose: () => void;
  } = $props();

  const PAGE = 50;
  const QUICK_REACTIONS = ["👍", "❤️", "😂", "😮", "😢", "🙏"];
  const EMOJIS = (
    "😀 😃 😄 😁 😆 😅 😂 🤣 😊 😇 🙂 😉 😍 🥰 😘 😋 😜 🤪 😎 🤩 🥳 😏 😒 😞 😔 😢 😭 😤 😠 😡 🤯 😳 🥺 😱 🤔 🤭 🤫 🙄 😴 🤤 " +
    "😷 🤒 👍 👎 👏 🙌 🙏 💪 👋 🤝 ✌️ 🤞 👌 👀 🔥 ✨ 🎉 💯 ✅ ❌ ⭐ ❤️ 🧡 💛 💚 💙 💜 🖤 💔"
  ).split(" ");
  // An original doodle tile, drawn as a mask in the theme's doodle color.
  const DOODLE = `<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="30" cy="30" r="10"/><path d="M120 40c-6-10-20-6-18 4 2 8 18 18 18 18s16-10 18-18c2-10-12-14-18-4z"/><path d="m200 20 4 9 10 1-7 7 2 10-9-5-9 5 2-10-7-7 10-1z"/><rect x="20" y="110" width="36" height="24" rx="8"/><path d="m30 134-4 8 10-8"/><circle cx="114" cy="146" r="6"/><path d="M120 146v-28l14-4v8"/><path d="M170 120c6-8 12 8 18 0s12 8 18 0 12 8 18 0"/><circle cx="60" cy="200" r="4"/><circle cx="60" cy="190" r="5"/><circle cx="70" cy="200" r="5"/><circle cx="60" cy="210" r="5"/><circle cx="50" cy="200" r="5"/><rect x="150" y="180" width="40" height="28" rx="6"/><circle cx="170" cy="194" r="8"/><circle cx="100" cy="200" r="2"/><circle cx="220" cy="90" r="2"/><path d="M90 70a14 14 0 1 0 14 18 11 11 0 0 1-14-18z"/></svg>`;
  const wallpaper = `url("data:image/svg+xml,${encodeURIComponent(DOODLE)}")`;
  // Primitive, so refreshing the chat list (a new `chat` object for the same
  // JID) does not rerun the effects below.
  const chatJid = $derived(chat.jid);
  const target = $derived({ accountId, chat: chatJid });

  let messages = $state<ChatMessage[]>([]);
  let exhausted = $state(false);
  let draft = $state("");
  let replyTo = $state<ChatMessage | null>(null);
  let editing = $state<ChatMessage | null>(null);
  let menu = $state<{ id: string; kind: "actions" | "react" } | null>(null);
  let chatMenu = $state(false);
  let emojiPicker = $state(false);
  let media = $state<Record<string, string>>({});
  let failure = $state("");
  let requestedOlder = $state(false);
  let atBottom = $state(true);
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
    const jid = chatJid;
    messages = [];
    exhausted = false;
    replyTo = null;
    editing = null;
    draft = "";
    requestedOlder = false;
    menu = null;
    chatMenu = false;
    evolution.messages({ accountId, chat: jid }, null, PAGE).then(async (page) => {
      if (jid !== chatJid) return;
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
    const jid = chatJid;
    const stick = nearBottom();
    evolution.messages({ accountId, chat: jid }, null, PAGE).then(async (page) => {
      if (jid !== chatJid) return;
      messages = mergeMessages(messages, page);
      if (stick) await scrollToBottom();
      if (document.visibilityState === "visible" && page.some((m) => !m.from_me)) {
        evolution.markRead({ accountId, chat: jid }).catch(() => {});
      }
    });
  });

  let loadingOlder = false;

  async function loadOlder() {
    if (loadingOlder || exhausted || messages.length === 0) return;
    const from = target;
    const before = messages[0].seq;
    const height = list.scrollHeight;
    loadingOlder = true;
    try {
      const page = await evolution.messages(from, before, PAGE);
      // The user may have switched chats meanwhile.
      if (from.chat !== chatJid || from.accountId !== accountId) return;
      exhausted = page.length < PAGE;
      messages = mergeMessages(messages, page);
      await tick();
      list.scrollTop = list.scrollHeight - height;
    } finally {
      loadingOlder = false;
    }
  }

  // The phone sends older messages later as a history sync.
  async function requestOlder() {
    chatMenu = false;
    failure = "";
    try {
      requestedOlder = await evolution.loadOlder(target);
    } catch (e) {
      failure = errorMessage(e);
    }
  }

  function onScroll() {
    atBottom = nearBottom();
    if (list.scrollTop < 40) loadOlder();
  }

  function stopTyping(to = target) {
    clearTimeout(typingTimer);
    if (composing) {
      composing = false;
      evolution.typing(to, false).catch(() => {});
    }
  }

  // The composer grows with its text, up to a few lines.
  function fitComposer() {
    if (!composer) return;
    composer.style.height = "auto";
    composer.style.height = `${Math.min(composer.scrollHeight, 140)}px`;
  }

  $effect(() => {
    void draft;
    tick().then(fitComposer);
  });

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
    emojiPicker = false;
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

  function insertEmoji(emoji: string) {
    const start = composer?.selectionStart ?? draft.length;
    const end = composer?.selectionEnd ?? draft.length;
    draft = draft.slice(0, start) + emoji + draft.slice(end);
    tick().then(() => {
      composer?.focus();
      composer?.setSelectionRange(start + emoji.length, start + emoji.length);
    });
    onInput();
  }

  function cancelContext() {
    if (editing) draft = "";
    replyTo = null;
    editing = null;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      submit();
    } else if (e.key === "Escape" && (replyTo || editing || emojiPicker)) {
      e.preventDefault();
      e.stopPropagation();
      if (emojiPicker) emojiPicker = false;
      else cancelContext();
    }
  }

  async function act(task: () => Promise<unknown>) {
    menu = null;
    failure = "";
    try {
      await task();
    } catch (e) {
      failure = errorMessage(e);
    }
  }

  function toggleMenu(id: string, kind: "actions" | "react") {
    menu = menu?.id === id && menu.kind === kind ? null : { id, kind };
  }

  function startReply(m: ChatMessage) {
    menu = null;
    editing = null;
    replyTo = m;
    composer?.focus();
  }

  function startEdit(m: ChatMessage) {
    menu = null;
    replyTo = null;
    editing = m;
    draft = m.body;
    composer?.focus();
  }

  function onWindowClick(e: MouseEvent) {
    const target = e.target as Element;
    if (menu && !target.closest?.(".menu, .more, .react")) menu = null;
    if (chatMenu && !target.closest?.(".chat-menu")) chatMenu = false;
    if (emojiPicker && !target.closest?.(".picker, .emoji-btn")) emojiPicker = false;
  }

  function openLink(e: MouseEvent, url: string) {
    e.preventDefault();
    openUrl(url).catch((err) => (failure = errorMessage(err)));
  }

  // Shown in the message itself: an expired file must not hide the chat.
  let mediaErrors = $state<Record<string, string>>({});

  async function loadMedia(m: ChatMessage) {
    try {
      media = { ...media, [m.id]: await evolution.media(target, m.id) };
      delete mediaErrors[m.id];
    } catch (e) {
      mediaErrors = { ...mediaErrors, [m.id]: errorMessage(e) };
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
  const visual = (m: ChatMessage) => !m.revoked && (m.kind === "image" || m.kind === "video" || m.kind === "sticker");
  const caption = (m: ChatMessage) => (m.body && !(m.kind === "document" && m.body === m.file_name) ? m.body : "");
</script>

<svelte:window onclick={onWindowClick} />

{#snippet meta(m: ChatMessage)}
  <span class="meta">
    {#if m.edited}<span>{t("edited")}</span>{/if}
    <span>{bubbleTime(m.ts, uiLocale())}</span>
    {#if m.from_me}
      <span class="tick" class:read={m.status === "read"} class:bad={statusIcon(m.status) === "alert"} title={m.status}>
        <Icon name={statusIcon(m.status)} size={16} />
      </span>
    {/if}
  </span>
{/snippet}

<section class="conversation">
  <header>
    <Avatar url={avatar} group={chat.is_group} size={40} />
    <div class="title">
      <span class="name">{chat.name}{#if chat.own}<span class="you">&nbsp;({t("You")})</span>{/if}</span>
      {#if typing === "composing"}<span class="typing">{t("typing…")}</span>
      {:else if typing === "recording"}<span class="typing">{t("recording audio…")}</span>{/if}
    </div>
    <div class="chat-menu">
      <button class="icon" aria-label={t("Menu")} aria-expanded={chatMenu} onclick={() => (chatMenu = !chatMenu)}>
        <Icon name="menu" />
      </button>
      {#if chatMenu}
        <div class="menu right" role="menu">
          {#if messages.length && online}
            <button role="menuitem" onclick={requestOlder}>{t("Load older messages from phone")}</button>
          {/if}
          <button role="menuitem" onclick={() => ((chatMenu = false), onclose())}>{t("Close chat")}</button>
        </div>
      {/if}
    </div>
  </header>

  <div class="pane" style:--wa-wallpaper={wallpaper}>
    <div class="messages" bind:this={list} onscroll={onScroll}>
      {#if exhausted}
        <div class="center">
          {#if requestedOlder}
            <span class="notice">{t("Older messages were requested from your phone and will appear when it answers.")}</span>
          {:else}
            <button class="notice" disabled={!online} onclick={requestOlder}>
              {t("Click this notice to load older messages from your phone.")}
            </button>
          {/if}
        </div>
      {/if}
      {#each messages as m, i (m.id)}
        {@const first = startsGroup(messages[i - 1], m)}
        {#if startsDay(messages[i - 1], m)}
          <div class="center"><span class="pill">{dayLabel(m.ts, uiLocale(), t)}</span></div>
        {/if}
        <div class="row" class:mine={m.from_me} class:first class:reacted={m.reactions.length > 0}>
          <div class="bubble" class:tail={first} class:visual={visual(m)} class:sticker={m.kind === "sticker" && !m.revoked}>
            {#if !m.revoked}
              <button class="more" aria-label={t("Message actions")} onclick={() => toggleMenu(m.id, "actions")}>
                <Icon name="down" size={20} />
              </button>
            {/if}
            {#if chat.is_group && !m.from_me && first}
              <span class="sender" style:color={senderColor(m.sender)}>{m.sender_name}</span>
            {/if}
            {#if m.quote}
              {@const q = m.quote}
              {@const color = q.from_me ? null : senderColor(q.sender)}
              <div class="quote" style:border-left-color={color}>
                <strong style:color>{q.from_me ? t("You") : q.sender_name}</strong>
                <span>{previewText(q.kind, q.text, t) || t("Quoted message")}</span>
              </div>
            {/if}
            {#if m.revoked}
              <p class="text deleted"><em>{t("This message was deleted")}</em>{@render meta(m)}</p>
            {:else}
              {#if m.kind === "image" || m.kind === "sticker"}
                {#if media[m.id]}
                  <img src={media[m.id]} alt={kindLabel(m.kind, t)} />
                {:else}
                  <div class="placeholder" use:autoload={m}>
                    <Icon name={m.kind === "sticker" ? "emoji" : "photo"} size={40} />
                    {#if mediaErrors[m.id]}<span class="media-error">{mediaErrors[m.id]}</span>{/if}
                  </div>
                {/if}
              {:else if m.kind === "video"}
                {#if media[m.id]}
                  <!-- svelte-ignore a11y_media_has_caption -->
                  <video controls src={media[m.id]}></video>
                {:else}
                  <button class="placeholder" onclick={() => loadMedia(m)} aria-label={kindLabel(m.kind, t)}>
                    <span class="play"><Icon name="play" size={32} /></span>
                    {#if mediaErrors[m.id]}<span class="media-error">{mediaErrors[m.id]}</span>{/if}
                  </button>
                {/if}
              {:else if m.kind === "audio"}
                {#if media[m.id]}
                  <audio controls src={media[m.id]}></audio>
                {:else}
                  <button class="voice" onclick={() => loadMedia(m)}>
                    <Icon name="play" size={28} />
                    <span class="wave"></span>
                    <Icon name="mic" size={20} />
                  </button>
                  {#if mediaErrors[m.id]}<span class="media-error">{mediaErrors[m.id]}</span>{/if}
                {/if}
              {:else if m.kind === "document"}
                <button class="doc" onclick={() => act(() => evolution.saveMedia(target, m.id))}>
                  <Icon name="document" size={30} />
                  <span class="doc-name">{m.file_name || kindLabel(m.kind, t)}</span>
                  <Icon name="download" size={22} />
                </button>
              {:else if m.kind !== "text"}
                <span class="kind">
                  <Icon name={m.kind === "location" ? "location" : m.kind === "contact" ? "person" : "alert"} size={18} />
                  {kindLabel(m.kind, t)}
                </span>
              {/if}
              {#if caption(m)}
                <p class="text">
                  {#each linkParts(caption(m)) as part, p (p)}{#if part.url}<a href={part.url} onclick={(e) => openLink(e, part.url!)}>{part.text}</a>{:else}{part.text}{/if}{/each}{@render meta(m)}
                </p>
              {:else if visual(m)}
                <span class="overlay">{@render meta(m)}</span>
              {:else}
                <p class="text">{@render meta(m)}</p>
              {/if}
            {/if}
          </div>
          {#if !m.revoked}
            <button class="react" aria-label={t("React")} onclick={() => toggleMenu(m.id, "react")}><Icon name="emoji" size={20} /></button>
          {/if}
          {#if m.reactions.length}
            <span class="reactions">
              {#each m.reactions as r (r.emoji)}<span class:own={r.mine}>{r.emoji}</span>{/each}
              {#if m.reactions.reduce((n, r) => n + r.count, 0) > 1}<span class="count">{m.reactions.reduce((n, r) => n + r.count, 0)}</span>{/if}
            </span>
          {/if}
          {#if menu?.id === m.id && menu.kind === "react"}
            <div class="menu tray" role="menu">
              {#each QUICK_REACTIONS as emoji (emoji)}
                <button role="menuitem" onclick={() => act(() => evolution.react(target, m.id, emoji))}>{emoji}</button>
              {/each}
              {#if m.reactions.some((r) => r.mine)}
                <button role="menuitem" title={t("Remove reaction")} onclick={() => act(() => evolution.react(target, m.id, ""))}>
                  <Icon name="close" size={18} />
                </button>
              {/if}
            </div>
          {:else if menu?.id === m.id}
            <div class="menu" role="menu">
              <button role="menuitem" onclick={() => startReply(m)}>{t("Reply")}</button>
              {#if m.body}<button role="menuitem" onclick={() => act(() => navigator.clipboard.writeText(m.body))}>{t("Copy")}</button>{/if}
              <button role="menuitem" onclick={() => toggleMenu(m.id, "react")}>{t("React")}</button>
              {#if m.kind !== "text" && m.kind !== "location" && m.kind !== "contact" && m.kind !== "unsupported"}
                <button role="menuitem" onclick={() => act(() => evolution.saveMedia(target, m.id))}>{t("Save as…")}</button>
              {/if}
              {#if canChange(m) && m.kind === "text"}<button role="menuitem" onclick={() => startEdit(m)}>{t("Edit")}</button>{/if}
              {#if m.from_me && (m.status === "failed" || m.status === "unknown") && m.kind === "text"}
                <button role="menuitem" onclick={() => act(() => evolution.retry(target, m.id))}>
                  {m.status === "unknown" ? t("Send again (it may already have been sent)") : t("Send again")}
                </button>
              {/if}
              {#if canChange(m)}
                <button role="menuitem" class="danger" onclick={() => act(() => evolution.remove(target, m.id))}>
                  {t("Delete for everyone")}
                </button>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>

    {#if !atBottom}
      <button class="to-bottom" aria-label={t("Scroll to the newest message")} onclick={scrollToBottom}>
        <Icon name="down" />
      </button>
    {/if}

    <footer>
      {#if failure}<p class="error" role="alert">{failure}</p>{/if}
      {#if emojiPicker}
        <div class="picker" role="listbox" aria-label={t("Emoji")}>
          {#each EMOJIS as emoji, e (e)}
            <button role="option" aria-selected="false" onclick={() => insertEmoji(emoji)}>{emoji}</button>
          {/each}
        </div>
      {/if}
      <div class="bar">
        {#if replyTo || editing}
          <div class="context">
            <div class="quote">
              <strong>{editing ? t("Editing message") : replyTo?.from_me ? t("You") : replyTo?.sender_name || chat.name}</strong>
              <span>{previewText((editing ?? replyTo)!.kind, (editing ?? replyTo)!.body, t)}</span>
            </div>
            <button class="icon" aria-label={t("Cancel")} onclick={cancelContext}><Icon name="close" /></button>
          </div>
        {/if}
        <div class="compose">
          <button class="icon" title={t("Attach file")} aria-label={t("Attach file")} disabled={!online || !!editing} onclick={attach}>
            <Icon name="attach" />
          </button>
          <button
            class="icon emoji-btn"
            aria-label={t("Emoji")}
            aria-expanded={emojiPicker}
            disabled={!online}
            onclick={() => (emojiPicker = !emojiPicker)}
          >
            <Icon name="emoji" />
          </button>
          <textarea
            bind:this={composer}
            rows="1"
            placeholder={online ? t("Type a message") : t("Offline: messages cannot be sent now")}
            disabled={!online}
            bind:value={draft}
            oninput={onInput}
            onkeydown={onKeydown}
          ></textarea>
          <button
            class="icon send"
            class:ready={!!draft.trim()}
            aria-label={editing ? t("Save") : t("Send")}
            disabled={!online || !draft.trim()}
            onclick={submit}
          >
            <Icon name={editing ? "check" : "send"} />
          </button>
        </div>
      </div>
    </footer>
  </div>
</section>

<style>
  .conversation {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    height: 100%;
    background: var(--wa-chat-bg);
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 64px;
    padding: 0 16px;
    border-bottom: 1px solid var(--wa-border);
    background: var(--wa-panel);
  }
  .title {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
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
  .typing {
    color: var(--wa-green);
    font-size: 13px;
  }
  .icon {
    display: grid;
    flex: none;
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
  .icon:hover:not(:disabled),
  .icon[aria-expanded="true"] {
    background: var(--wa-active);
  }
  .icon:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .chat-menu {
    position: relative;
  }
  .pane {
    position: relative;
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
  }
  .pane::before {
    position: absolute;
    inset: 0;
    background: var(--wa-doodle);
    content: "";
    pointer-events: none;
    -webkit-mask-image: var(--wa-wallpaper);
    mask-image: var(--wa-wallpaper);
  }
  .messages {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px clamp(16px, 7%, 72px) 8px;
  }
  .center {
    display: flex;
    justify-content: center;
    margin: 12px 0 8px;
  }
  .pill,
  .notice {
    padding: 6px 12px;
    border-radius: 8px;
    background: var(--wa-pill);
    box-shadow: var(--wa-bubble-shadow);
    color: var(--wa-secondary);
    font-size: 12.5px;
  }
  .notice {
    max-width: 480px;
    border: 0;
    background: var(--wa-notice);
    color: var(--wa-notice-text);
    text-align: center;
  }
  button.notice {
    cursor: pointer;
  }
  button.notice:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
  }
  .row.first {
    margin-top: 10px;
  }
  .row.reacted {
    margin-bottom: 18px;
  }
  .row.mine {
    flex-direction: row-reverse;
  }
  .bubble {
    position: relative;
    max-width: min(560px, 65%);
    padding: 6px 7px 8px 9px;
    border-radius: 8px;
    background: var(--wa-bubble-in);
    box-shadow: var(--wa-bubble-shadow);
    overflow-wrap: anywhere;
  }
  .mine .bubble {
    background: var(--wa-bubble-out);
  }
  .bubble.tail {
    border-top-left-radius: 0;
  }
  .mine .bubble.tail {
    border-top-left-radius: 8px;
    border-top-right-radius: 0;
  }
  .bubble.tail::before {
    position: absolute;
    top: 0;
    left: -8px;
    width: 8px;
    height: 13px;
    background: inherit;
    clip-path: polygon(0 0, 100% 0, 100% 100%);
    content: "";
  }
  .mine .bubble.tail::before {
    right: -8px;
    left: auto;
    clip-path: polygon(0 0, 100% 0, 0 100%);
  }
  .bubble.visual {
    padding: 3px;
  }
  .bubble.sticker {
    background: none;
    box-shadow: none;
  }
  .bubble.sticker::before {
    display: none;
  }
  .more {
    position: absolute;
    top: 2px;
    right: 2px;
    z-index: 2;
    display: grid;
    padding: 0 0 4px 12px;
    border: 0;
    border-radius: 0 8px 0 16px;
    background: radial-gradient(at top right, var(--wa-bubble-in) 60%, transparent 80%);
    color: var(--wa-secondary);
    opacity: 0;
    cursor: pointer;
  }
  .mine .more {
    background: radial-gradient(at top right, var(--wa-bubble-out) 60%, transparent 80%);
  }
  .bubble.visual .more {
    background: none;
    color: #fff;
    filter: drop-shadow(0 0 2px rgb(0 0 0 / 60%));
  }
  .row:hover .more,
  .row:hover .react {
    opacity: 1;
  }
  .react {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: rgb(0 0 0 / 15%);
    color: var(--wa-text);
    opacity: 0;
    cursor: pointer;
  }
  .sender {
    display: block;
    margin-bottom: 2px;
    font-size: 13px;
    font-weight: 600;
  }
  .quote {
    margin: 0 0 4px;
    padding: 6px 8px;
    border-left: 4px solid var(--wa-green);
    border-radius: 6px;
    background: var(--wa-quote-bg);
    color: var(--wa-secondary);
    font-size: 13px;
  }
  .bubble .quote {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .bubble .quote strong {
    color: var(--wa-green);
    font-weight: 600;
  }
  .bubble .quote span {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .text {
    /* Holds the floated time, which sits on the last line when it fits. */
    display: flow-root;
    margin: 0;
    font-size: 14.2px;
    line-height: 19px;
    white-space: pre-wrap;
  }
  .text a {
    color: var(--wa-tick-read);
  }
  .deleted {
    color: var(--wa-secondary);
  }
  .meta {
    position: relative;
    top: 5px;
    float: right;
    display: flex;
    align-items: center;
    gap: 3px;
    margin: 0 0 0 10px;
    color: var(--wa-meta);
    font-size: 11px;
    line-height: 15px;
    white-space: nowrap;
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
  .overlay {
    position: absolute;
    right: 8px;
    bottom: 6px;
    padding: 0 4px;
    border-radius: 6px;
    background: rgb(0 0 0 / 35%);
  }
  .overlay .meta {
    top: 0;
    margin: 0;
    color: #fff;
  }
  .overlay .tick.read {
    color: var(--wa-tick-read);
  }
  .bubble img,
  .bubble video {
    display: block;
    max-width: min(330px, 100%);
    max-height: 330px;
    border-radius: 6px;
  }
  .bubble.sticker img {
    max-width: 160px;
  }
  .placeholder {
    display: grid;
    place-content: center;
    place-items: center;
    gap: 6px;
    width: 240px;
    height: 180px;
    padding: 8px;
    border: 0;
    border-radius: 6px;
    background: var(--wa-quote-bg);
    color: var(--wa-secondary);
  }
  button.placeholder {
    cursor: pointer;
  }
  .play {
    display: grid;
    place-items: center;
    width: 52px;
    height: 52px;
    border-radius: 50%;
    background: rgb(0 0 0 / 45%);
    color: #fff;
  }
  .media-error {
    font-size: 12px;
    text-align: center;
  }
  .voice {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 260px;
    padding: 6px 4px;
    border: 0;
    background: none;
    color: var(--wa-secondary);
    cursor: pointer;
  }
  .wave {
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: currentColor;
    opacity: 0.5;
  }
  audio {
    display: block;
    width: 280px;
    max-width: 100%;
  }
  .doc {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 240px;
    margin-bottom: 4px;
    padding: 10px;
    border: 0;
    border-radius: 6px;
    background: var(--wa-quote-bg);
    color: var(--wa-text);
    text-align: left;
    cursor: pointer;
  }
  .doc-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--wa-secondary);
  }
  .reactions {
    position: absolute;
    bottom: -18px;
    left: 10px;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 1px 6px;
    border: 2px solid var(--wa-chat-bg);
    border-radius: 12px;
    background: var(--wa-pill);
    font-size: 14px;
  }
  .mine .reactions {
    right: 10px;
    left: auto;
  }
  .reactions .count {
    color: var(--wa-secondary);
    font-size: 12px;
  }
  .menu {
    position: absolute;
    top: 28px;
    z-index: 10;
    display: flex;
    flex-direction: column;
    min-width: 200px;
    padding: 8px;
    border-radius: 16px;
    background: var(--wa-menu);
    box-shadow: var(--wa-menu-shadow);
  }
  .row:not(.mine) .menu {
    left: 24px;
  }
  .row.mine .menu,
  .menu.right {
    right: 0;
  }
  .menu.right {
    top: 44px;
  }
  .menu.tray {
    top: -46px;
    flex-direction: row;
    min-width: 0;
    padding: 4px 8px;
    border-radius: 24px;
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
  .menu.tray button {
    display: grid;
    place-items: center;
    padding: 4px 6px;
    border-radius: 50%;
    font-size: 22px;
  }
  .menu button:hover {
    background: var(--wa-hover);
  }
  .menu .danger {
    color: var(--wa-danger);
  }
  .to-bottom {
    position: absolute;
    right: 24px;
    bottom: 84px;
    z-index: 3;
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border: 0;
    border-radius: 50%;
    background: var(--wa-pill);
    box-shadow: var(--wa-menu-shadow);
    color: var(--wa-secondary);
    cursor: pointer;
  }
  footer {
    position: relative;
    padding: 4px 16px 12px;
  }
  .error {
    margin: 0 0 6px;
    padding: 6px 12px;
    border-radius: 8px;
    background: var(--wa-pill);
    color: var(--wa-danger);
    font-size: 13px;
  }
  .picker {
    position: absolute;
    bottom: 72px;
    left: 16px;
    z-index: 5;
    display: grid;
    grid-template-columns: repeat(10, 36px);
    gap: 2px;
    max-height: 240px;
    padding: 10px;
    overflow-y: auto;
    border-radius: 16px;
    background: var(--wa-menu);
    box-shadow: var(--wa-menu-shadow);
  }
  .picker button {
    width: 36px;
    height: 36px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: none;
    font-size: 22px;
    cursor: pointer;
  }
  .picker button:hover {
    background: var(--wa-hover);
  }
  .bar {
    border-radius: 26px;
    background: var(--wa-pill);
    box-shadow: var(--wa-bubble-shadow);
  }
  .context {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 0 12px;
  }
  .context .quote {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    margin: 0;
  }
  .context .quote strong {
    color: var(--wa-green);
  }
  .context .quote span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .compose {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    padding: 6px 6px;
  }
  textarea {
    /* The auto height comes from scrollHeight, which includes the padding. */
    box-sizing: border-box;
    flex: 1;
    height: 40px;
    min-height: 40px;
    margin: 0 6px;
    padding: 9px 0;
    border: 0;
    outline: 0;
    background: none;
    color: var(--wa-text);
    font: inherit;
    font-size: 15px;
    line-height: 22px;
    resize: none;
  }
  textarea::placeholder {
    color: var(--wa-secondary);
  }
  .send.ready {
    background: var(--wa-green);
    color: var(--wa-on-green);
  }
  .send.ready:hover {
    background: var(--wa-green);
  }
</style>
