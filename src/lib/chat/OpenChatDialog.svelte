<script lang="ts">
  import { onMount } from "svelte";

  import { t, uiLocale } from "$lib/i18n/index.svelte";
  import { app, type RegionCallingCode } from "$lib/ipc";

  import { defaultRegion, MAX_MESSAGE_LENGTH, validateChat, type ChatError } from "./validate";

  // ZapZap's "Send message to number" dialog.
  let { accountId, onclose }: { accountId: number | null; onclose: () => void } = $props();

  type Country = { region: string; code: string; label: string };
  let countries = $state<Country[]>([]);
  let region = $state("");
  let number = $state("");
  let message = $state("");
  let touched = $state(false);
  let failure = $state("");

  onMount(async () => {
    let names: Intl.DisplayNames | null = null;
    try {
      names = new Intl.DisplayNames([uiLocale()], { type: "region" });
    } catch {
      names = null;
    }
    const codes: RegionCallingCode[] = await app.callingCodes();
    countries = codes
      .map(({ region, calling_code }) => {
        const name = names?.of(region) ?? region;
        return { region, code: calling_code, label: `${name} (${region})  +${calling_code}` };
      })
      .sort((a, b) => a.label.localeCompare(b.label, uiLocale()));
    const preferred = defaultRegion([uiLocale(), ...navigator.languages]);
    region = countries.find((c) => c.region === preferred)?.region ?? "";
  });

  const country = $derived(countries.find((c) => c.region === region));
  const error = $derived(validateChat(country?.code ?? "", number, message));
  const shownError = $derived(error && (touched || error === "missing_country") ? error : null);

  function errorText(code: ChatError): string {
    return {
      missing_country: t("Select a country code."),
      empty_number: t("Enter a phone number."),
      number_too_short: t("The phone number appears to be incomplete."),
      number_too_long: t("The phone number exceeds the international limit."),
      duplicated_country_code: t("Do not include the country code again in this field."),
      message_too_long: t("The message can contain up to 500 characters."),
    }[code];
  }

  async function send() {
    touched = true;
    if (error || !country) return;
    try {
      await app.openChatByNumber({ country_code: country.code, national_number: number, message }, accountId ?? undefined);
      onclose();
    } catch (e) {
      failure = (e as { message?: string })?.message ?? String(e);
    }
  }
</script>

<svelte:window
  onkeydowncapture={(e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }}
/>

<div class="backdrop">
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Send message to number")}>
    <header>
      <div>
        <h2>{t("Send message to number")}</h2>
        <p class="muted">{t("Chat with someone who is not in your contact list.")}</p>
      </div>
      <button class="close" title={t("Close")} aria-label={t("Close")} onclick={onclose}>✕</button>
    </header>

    <div class="fields">
      <label>
        {t("Country code")}
        <select bind:value={region} aria-label={t("Country code")}>
          <option value="" disabled>—</option>
          {#each countries as c (c.region)}<option value={c.region}>{c.label}</option>{/each}
        </select>
      </label>
      <label>
        {t("Phone number")}
        <!-- svelte-ignore a11y_autofocus -->
        <input
          autofocus
          inputmode="tel"
          placeholder={t("Example: 11 99999-9999")}
          aria-label={t("Phone number")}
          aria-invalid={shownError !== null}
          bind:value={number}
          oninput={() => (touched = true)}
          onkeydown={(e) => e.key === "Enter" && send()}
        />
      </label>
    </div>
    <p class="hint muted">{t("Enter the national number without the country code.")}</p>
    {#if shownError}<p class="error" role="alert" aria-label={t("Phone number error")}>{errorText(shownError)}</p>{/if}

    <label class="message">
      <span class="row">
        {t("Optional message")}
        <span class="muted" aria-label={t("Message character count")}>{[...message].length}/{MAX_MESSAGE_LENGTH}</span>
      </span>
      <textarea
        rows="4"
        maxlength={MAX_MESSAGE_LENGTH}
        placeholder={t("Hello! How are you?")}
        bind:value={message}
        onkeydown={(e) => e.key === "Enter" && e.ctrlKey && send()}
      ></textarea>
    </label>

    <div class="info-box">
      <strong>{t("Your privacy is protected")}</strong><br />
      {t("The message will be sent directly to the number provided.")}
    </div>
    {#if failure}<p class="error" role="alert">{failure}</p>{/if}

    <footer>
      <button class="btn" onclick={onclose}>{t("Cancel")}</button>
      <button class="btn primary" disabled={error !== null} onclick={send}>{t("Send message")}</button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 40%);
  }
  .dialog {
    width: min(520px, calc(100vw - 32px));
    padding: 16px 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
    box-sizing: border-box;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
  }
  h2 {
    margin: 0;
    font-size: 17px;
  }
  p {
    margin: 4px 0;
  }
  .close {
    border: 0;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-top: 12px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .message {
    margin-top: 10px;
  }
  .row {
    display: flex;
    justify-content: space-between;
  }
  textarea {
    resize: vertical;
    font: inherit;
  }
  .hint {
    font-size: 12px;
  }
  .error {
    color: var(--danger, #c0392b);
    font-size: 13px;
  }
  .info-box {
    margin-top: 10px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
</style>
