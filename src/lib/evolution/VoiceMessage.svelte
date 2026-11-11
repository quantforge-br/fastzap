<script lang="ts">
  import { untrack, type Snippet } from "svelte";

  import { t, uiLocale } from "$lib/i18n/index.svelte";
  import type { ChatMessage } from "$lib/ipc";

  import Avatar from "./Avatar.svelte";
  import Icon from "./Icon.svelte";
  import { formatDuration, nextSpeed, progress, speedLabel, waveformBars } from "./voice";

  // A voice message player drawn like WhatsApp's: play button, waveform with
  // a seek dot, elapsed time, and the sender's photo (the speed while playing).
  let {
    message,
    src,
    error = "",
    avatar = null,
    active,
    autostart,
    onload,
    onplay,
    onended,
    meta,
  }: {
    message: ChatMessage;
    /** Media URL once loaded. */
    src?: string;
    error?: string;
    avatar?: string | null;
    /** This is the conversation's current audio; any other one pauses. */
    active: boolean;
    /** Start playing as soon as the media is available. */
    autostart: boolean;
    onload: () => void;
    onplay: () => void;
    onended: () => void;
    /** Time and delivery status of the message. */
    meta: Snippet<[ChatMessage]>;
  } = $props();

  let audio = $state<HTMLAudioElement>();
  let paused = $state(true);
  let currentTime = $state(0);
  let mediaDuration = $state(0);
  let rate = $state(1);
  let loading = $state(false);
  /** Play (from this fraction) once the media is loaded. */
  let pending = $state<number | null>(null);
  let wave = $state<HTMLElement>();
  let dragging = $state(false);

  const bars = $derived(waveformBars(message.waveform));
  // WebKit reports Infinity for streamed Opus; the message knows the length.
  const total = $derived(Number.isFinite(mediaDuration) && mediaDuration > 0 ? mediaDuration : message.seconds);
  const done = $derived(progress(currentTime, total));
  const started = $derived(!paused || currentTime > 0);
  const label = $derived(formatDuration(started ? currentTime : total));

  $effect(() => {
    if (error) loading = false;
  });

  // Another audio took over: stop this one.
  $effect(() => {
    if (!active && audio && !audio.paused) audio.pause();
  });

  $effect(() => {
    if (autostart) untrack(() => request(0));
  });

  // The media arrived: start where the user asked.
  $effect(() => {
    if (src && audio && pending !== null) {
      const from = pending;
      pending = null;
      loading = false;
      untrack(() => start(from));
    }
  });

  function request(from: number) {
    if (src && audio) return start(from);
    pending = from;
    if (!loading) {
      loading = true;
      onload();
    }
  }

  function start(from: number) {
    if (!audio) return;
    if (from > 0 && total > 0) audio.currentTime = from * total;
    audio.playbackRate = rate;
    audio.play().catch(() => {});
  }

  function toggle() {
    if (!src || !audio) return request(0);
    if (audio.paused) start(0);
    else audio.pause();
  }

  function fractionAt(e: PointerEvent): number {
    const box = wave!.getBoundingClientRect();
    return box.width ? Math.min(1, Math.max(0, (e.clientX - box.left) / box.width)) : 0;
  }

  function seek(fraction: number) {
    if (!src || !audio) return request(fraction);
    if (total > 0) audio.currentTime = fraction * total;
    currentTime = audio.currentTime;
  }

  function pointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    dragging = true;
    wave!.setPointerCapture(e.pointerId);
    seek(fractionAt(e));
  }

  function pointerMove(e: PointerEvent) {
    if (dragging) seek(fractionAt(e));
  }

  function pointerUp(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    wave!.releasePointerCapture(e.pointerId);
  }

  function keydown(e: KeyboardEvent) {
    const step = e.key === "ArrowRight" ? 5 : e.key === "ArrowLeft" ? -5 : 0;
    if (!step || !audio || !(total > 0)) return;
    e.preventDefault();
    seek(Math.min(1, Math.max(0, (audio.currentTime + step) / total)));
  }

  function changeSpeed() {
    rate = nextSpeed(rate);
    if (audio) audio.playbackRate = rate;
  }

  function ended() {
    currentTime = 0;
    onended();
  }
</script>

<div class="voice" class:mine={message.from_me}>
  {#if src}
    <audio
      bind:this={audio}
      {src}
      preload="metadata"
      bind:paused
      bind:currentTime
      bind:duration={mediaDuration}
      onplay={onplay}
      onended={ended}
    ></audio>
  {/if}
  <div class="side">
    {#if started}
      <button class="speed" onclick={changeSpeed} aria-label={t("Playback speed")}>{speedLabel(rate, uiLocale())}</button>
    {:else}
      <span class="photo">
        <Avatar url={avatar} size={55} />
        {#if message.voice}<span class="mic"><Icon name="mic" size={20} /></span>{/if}
      </span>
    {/if}
  </div>
  <div class="player">
    <button class="play" onclick={toggle} aria-label={paused ? t("Play") : t("Pause")}>
      {#if loading && !src}
        <span class="spinner"></span>
      {:else}
        <Icon name={paused ? "play" : "pause"} size={34} />
      {/if}
    </button>
    <div class="track">
      <div
        class="wave"
        role="slider"
        tabindex="0"
        aria-label={t("Audio position")}
        aria-valuemin={0}
        aria-valuemax={Math.round(total)}
        aria-valuenow={Math.round(currentTime)}
        bind:this={wave}
        onpointerdown={pointerDown}
        onpointermove={pointerMove}
        onpointerup={pointerUp}
        onpointercancel={pointerUp}
        onkeydown={keydown}
      >
        {#each bars as height, i (i)}
          <span class="bar" class:played={(i + 0.5) / bars.length <= done} style:height="{height * 100}%"></span>
        {/each}
        <span class="dot" style:left="{done * 100}%"></span>
      </div>
      <div class="info">
        <span class="time">{label}</span>
        {@render meta(message)}
      </div>
    </div>
  </div>
</div>
{#if error}<span class="media-error">{error}</span>{/if}

<style>
  .voice {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 336px;
    max-width: 100%;
    padding: 4px 2px 0;
  }
  /* WhatsApp puts the photo after the player in received audios. */
  .voice:not(.mine) {
    flex-direction: row-reverse;
  }
  .side {
    display: grid;
    flex: none;
    place-items: center;
    width: 55px;
    height: 55px;
  }
  .photo {
    position: relative;
    display: block;
  }
  .mic {
    position: absolute;
    bottom: -2px;
    left: -4px;
    display: grid;
    color: var(--wa-green);
    filter: drop-shadow(0 0 1px var(--wa-bubble-in));
  }
  .mine .mic {
    right: -4px;
    left: auto;
    color: var(--wa-tick-read);
    filter: drop-shadow(0 0 1px var(--wa-bubble-out));
  }
  .speed {
    min-width: 46px;
    height: 26px;
    padding: 0 8px;
    border: 0;
    border-radius: 13px;
    background: rgb(0 0 0 / 25%);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .player {
    display: flex;
    flex: 1;
    min-width: 0;
    align-items: flex-start;
    gap: 6px;
  }
  .play {
    display: grid;
    flex: none;
    place-items: center;
    width: 34px;
    height: 34px;
    margin-top: 2px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--wa-icon);
    cursor: pointer;
  }
  .spinner {
    width: 22px;
    height: 22px;
    border: 2px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .track {
    flex: 1;
    min-width: 0;
  }
  .wave {
    position: relative;
    display: flex;
    align-items: center;
    gap: 2px;
    height: 38px;
    cursor: pointer;
    touch-action: none;
    outline-offset: 2px;
  }
  .bar {
    flex: 1;
    min-height: 3px;
    max-height: 26px;
    border-radius: 2px;
    background: var(--wa-meta);
    opacity: 0.55;
  }
  .bar.played {
    background: var(--wa-tick-read);
    opacity: 1;
  }
  .dot {
    position: absolute;
    top: 50%;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    background: var(--wa-tick-read);
    transform: translate(-50%, -50%);
    pointer-events: none;
  }
  .mine .dot,
  .mine .bar.played {
    background: var(--wa-green);
  }
  .info {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    margin-top: -4px;
    color: var(--wa-meta);
    font-size: 11px;
  }
  .info :global(.meta) {
    top: 0;
    float: none;
    margin: 0;
  }
  .media-error {
    display: block;
    font-size: 12px;
    text-align: center;
  }
</style>
