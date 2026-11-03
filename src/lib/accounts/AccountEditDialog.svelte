<script lang="ts">
  import { untrack } from "svelte";

  // ZapZap's "Edit account" dialog: name, icon (generated or photo) and
  // User-Agent, saved together with Save.
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { accounts as accountsApi, type Account } from "$lib/ipc";

  import { DEFAULT_ICON, dataUrlToBlob, iconSrc, newIconSvg, parseIcon, photoFromBlob, serializeIcon } from "./icon";

  let { account, onclose }: { account: Account; onclose: () => void } = $props();

  // The form edits a snapshot; `account` stays live only for photo syncs.
  const initial = untrack(() => account);
  const original = parseIcon(initial.icon);
  let name = $state(initial.name);
  let choice = $state({ ...original });
  let userAgent = $state(initial.user_agent);
  let presets = $state<string[]>([]);
  let feedback = $state("");
  let error = $state("");
  let discardOpen = $state(false);
  let userAgentNotice = $state(false);
  let syncing = $state(false);

  const preview = $derived(iconSrc(serializeIcon(choice)));
  const valid = $derived(name.trim().length > 0);
  const dirty = $derived(
    name !== initial.name ||
      userAgent !== initial.user_agent ||
      serializeIcon(choice) !== serializeIcon(original),
  );

  accountsApi.userAgentPresets().then((list) => (presets = list));

  // A profile photo sync is saved by the backend; pick up the new photo.
  $effect(() => {
    if (!syncing) return;
    const synced = parseIcon(account.icon);
    if (synced.photo && synced.photo !== choice.photo) {
      choice = { ...choice, photo: synced.photo, usePhoto: true };
      syncing = false;
      feedback = t("Preview image updated.");
    }
  });

  function setMode(usePhoto: boolean) {
    choice = { ...choice, usePhoto };
  }

  function newColors() {
    choice = { ...choice, icon: newIconSvg(), usePhoto: false };
    feedback = t("New icon colors selected.");
  }

  function standardIcon() {
    choice = { ...choice, icon: DEFAULT_ICON, usePhoto: false };
    feedback = t("Standard icon selected.");
  }

  function keepCurrent() {
    choice = { ...original };
    feedback = "";
  }

  async function choosePhoto() {
    try {
      const picked = await accountsApi.pickPhoto(t("Choose photo"));
      if (!picked) return;
      choice = { ...choice, photo: await photoFromBlob(dataUrlToBlob(picked)), usePhoto: true };
      feedback = t("Preview image updated.");
    } catch {
      feedback = t("Could not load preview image.");
    }
  }

  async function syncPhoto() {
    syncing = true;
    feedback = "";
    await accountsApi.syncPhoto(account.id);
  }

  async function save() {
    if (!valid) return;
    try {
      await accountsApi.update(account.id, {
        name: name.trim(),
        icon: serializeIcon(choice),
        enabled: null,
        zoom: null,
        user_agent: userAgent === initial.user_agent ? null : userAgent,
        notifications: null,
        evolution: null,
      });
    } catch {
      error = t("The account could not be updated. Please try again.");
      return;
    }
    if (userAgent !== initial.user_agent) userAgentNotice = true;
    else onclose();
  }

  function cancel() {
    if (dirty) discardOpen = true;
    else onclose();
  }
</script>

<svelte:window
  onkeydowncapture={(e) => {
    if (e.key === "Escape" && !discardOpen && !userAgentNotice) {
      e.preventDefault();
      cancel();
    }
  }}
/>

<div class="backdrop">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="edit-title">
    <h2 id="edit-title">{t("Edit account")}</h2>
    <p class="muted">{t("Change the name and icon used to identify this account.")}</p>

    <label class="field">
      <span>{t("Account name")}</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:value={name} autofocus maxlength="64" />
      {#if !valid}<span class="error">{t("Account name is required.")}</span>{/if}
    </label>

    <div class="field">
      <span>{t("Account icon")}</span>
      <div class="icon-row">
        <img class="preview" src={preview} alt={t("Current icon")} title={t("Current icon")} />
        <div class="icon-controls">
          <div class="segmented" role="radiogroup" aria-label={t("Account icon")}>
            <button role="radio" aria-checked={!choice.usePhoto} onclick={() => setMode(false)}>
              {t("Default")}
            </button>
            <button
              role="radio"
              aria-checked={choice.usePhoto}
              disabled={!choice.photo}
              onclick={() => setMode(true)}
            >
              {t("Photo")}
            </button>
          </div>
          <div class="buttons">
            <button class="btn" onclick={newColors}>{t("Generate new colors for the icon")}</button>
            <button class="btn" onclick={standardIcon}>{t("Restore standard")}</button>
            <button class="btn" onclick={choosePhoto}>{t("Choose photo")}</button>
            <button class="btn" disabled={!account.enabled || syncing} onclick={syncPhoto}>
              {t("Sync profile photo")}
            </button>
            <button class="btn" onclick={keepCurrent}>{t("Keep current icon")}</button>
          </div>
          {#if feedback}<span class="muted">{feedback}</span>{/if}
        </div>
      </div>
    </div>

    <details>
      <summary>{t("Advanced options")}</summary>
      <label class="field">
        <span title={t("Changes the identification used by this account when loading pages.")}>
          {t("User-Agent")} ⓘ
        </span>
        <select bind:value={userAgent}>
          {#each presets as preset (preset)}
            <option value={preset}>{t(preset)}</option>
          {/each}
        </select>
      </label>
    </details>

    {#if error}<p class="info-box warning">{error}</p>{/if}

    <div class="actions">
      <button class="btn" onclick={cancel}>{t("Cancel")}</button>
      <button class="btn primary" disabled={!valid} onclick={save}>{t("Save")}</button>
    </div>
  </div>
</div>

{#if discardOpen}
  <ConfirmDialog
    title={t("Discard account changes?")}
    message={t("There are unsaved changes to this account.")}
    detail={t("Discarding them restores the values from before editing.")}
    confirmLabel={t("Discard")}
    cancelLabel={t("Keep editing")}
    danger
    onconfirm={onclose}
    oncancel={() => (discardOpen = false)}
  />
{/if}

{#if userAgentNotice}
  <ConfirmDialog
    title={t("User-Agent Changed")}
    message={t("Please restart this session (or the application) to apply the new User-Agent.")}
    confirmLabel={t("OK")}
    onconfirm={onclose}
    oncancel={onclose}
  />
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 40%);
  }
  .dialog {
    width: min(520px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow-y: auto;
    padding: 20px 24px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
  }
  h2 {
    margin: 0 0 4px;
    font-size: 17px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 16px;
  }
  .field > span:first-child {
    font-weight: 600;
    font-size: 13px;
  }
  .error {
    color: #c01c28;
    font-size: 12px;
  }
  .icon-row {
    display: flex;
    gap: 16px;
  }
  .preview {
    width: 72px;
    height: 72px;
    flex: none;
    border-radius: 50%;
  }
  .icon-controls {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .segmented {
    display: inline-flex;
    align-self: flex-start;
    border: 1px solid var(--csr-mid);
    border-radius: 8px;
    overflow: hidden;
  }
  .segmented button {
    padding: 4px 12px;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .segmented button[aria-checked="true"] {
    background: var(--accent);
    color: #fff;
  }
  .segmented button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  details {
    margin-top: 16px;
  }
  summary {
    cursor: pointer;
    font-size: 13px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 20px;
  }
</style>
