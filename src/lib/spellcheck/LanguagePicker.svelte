<script lang="ts">
  import { untrack } from "svelte";

  import { t } from "$lib/i18n/index.svelte";
  import type { SpellState } from "$lib/ipc";

  import { dictionaryLabel, matches, MAX_LANGUAGES, searchKey } from "./format";

  // ZapZap's language picker: changes are staged until Apply.
  let {
    spell,
    onapply,
    oncancel,
    onmanage,
  }: {
    spell: SpellState;
    onapply: (languages: string[]) => void;
    oncancel: () => void;
    onmanage: () => void;
  } = $props();

  const original = untrack(() => [...spell.languages]);
  let selected = $state([...original]);
  let query = $state("");
  let feedback = $state("");

  const options = $derived(
    spell.installed
      .map((d) => ({ tag: d.tag, label: dictionaryLabel(d.tag) }))
      .sort((a, b) => a.label.localeCompare(b.label) || a.tag.localeCompare(b.tag)),
  );
  const filtered = $derived(options.filter((o) => matches(query, o.label, o.tag)));
  const recent = $derived(spell.recent.filter((tag) => !selected.includes(tag)));
  const full = $derived(selected.length >= MAX_LANGUAGES);
  const changed = $derived(selected.join("\n") !== original.join("\n"));

  function toggle(tag: string) {
    feedback = "";
    if (selected.includes(tag)) {
      if (selected.length === 1) {
        feedback = t("At least one language must remain selected.");
        return;
      }
      selected = selected.filter((s) => s !== tag);
    } else if (full) {
      feedback = t("You can select up to 10 languages.");
    } else {
      selected = [...selected, tag];
    }
  }

  function letter(label: string) {
    const first = searchKey(label).charAt(0).toUpperCase();
    return /[A-Z]/.test(first) ? first : "#";
  }
</script>

<svelte:window
  onkeydowncapture={(e) => {
    if (e.key !== "Escape") return;
    e.preventDefault();
    if (query) query = "";
    else oncancel();
  }}
/>

<div class="backdrop">
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Languages")}>
    <header>
      <h2>{t("Languages")}</h2>
      <button class="close" title={t("Close")} aria-label={t("Close")} onclick={oncancel}>✕</button>
    </header>

    {#if options.length === 0}
      <p class="empty">
        <strong>{t("No dictionary available")}</strong><br />
        {t("Install or configure a dictionary to use the spell checker.")}
      </p>
    {:else}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="search"
        type="search"
        autofocus
        placeholder={t("Search language")}
        aria-label={t("Search language")}
        bind:value={query}
      />

      <div class="heading">
        <span>{t("Selected languages")}</span>
        <span class="muted">{selected.length}/{MAX_LANGUAGES}</span>
      </div>
      <div class="chips">
        {#each selected as tag (tag)}
          <button
            class="chip"
            aria-label={t("Remove {language}").replace("{language}", dictionaryLabel(tag))}
            onclick={() => toggle(tag)}>{dictionaryLabel(tag)} ×</button
          >
        {/each}
      </div>
      <p class="feedback" role="status" aria-label={t("Language selection status")}>{feedback}</p>

      {#if recent.length && !query}
        <div class="heading"><span>{t("Recent")}</span></div>
        <div class="chips">
          {#each recent as tag (tag)}
            <button
              class="chip"
              disabled={full}
              aria-label={t("Add {language}").replace("{language}", dictionaryLabel(tag))}
              onclick={() => toggle(tag)}>+ {dictionaryLabel(tag)}</button
            >
          {/each}
        </div>
      {/if}

      <div class="heading"><span>{t("All languages")}</span></div>
      <ul class="list">
        {#each filtered as option, i (option.tag)}
          {#if !query && (i === 0 || letter(filtered[i - 1].label) !== letter(option.label))}
            <li class="letter" aria-hidden="true">{letter(option.label)}</li>
          {/if}
          {@const checked = selected.includes(option.tag)}
          <li>
            <label class:disabled={full && !checked} title={full && !checked ? t("You can select up to 10 languages.") : ""}>
              <input type="checkbox" {checked} disabled={full && !checked} onchange={() => toggle(option.tag)} />
              <span>{option.label}</span>
              <span class="muted code">{option.tag}</span>
            </label>
          </li>
        {:else}
          <li class="empty">{t("No language found")}</li>
        {/each}
      </ul>
    {/if}

    <footer>
      {#if spell.supported}
        <button class="btn" onclick={onmanage}>{t("Manage dictionaries")}</button>
      {/if}
      <span class="spacer"></span>
      <button class="btn" onclick={oncancel}>{t("Cancel")}</button>
      <button class="btn primary" disabled={!selected.length || !changed} onclick={() => onapply(selected)}>
        {t("Apply")}
      </button>
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
    display: flex;
    flex-direction: column;
    width: min(680px, calc(100vw - 32px));
    height: min(700px, calc(100vh - 32px));
    padding: 16px 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
    box-sizing: border-box;
  }
  header,
  footer,
  .heading {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  header {
    justify-content: space-between;
    margin-bottom: 8px;
  }
  h2 {
    margin: 0;
    font-size: 17px;
  }
  .close {
    border: 0;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .search {
    width: 100%;
    box-sizing: border-box;
  }
  .heading {
    justify-content: space-between;
    margin-top: 12px;
    font-weight: 600;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 6px;
  }
  .chip {
    padding: 2px 10px;
    border: 1px solid var(--csr-mid);
    border-radius: 12px;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .feedback {
    min-height: 1.2em;
    margin: 4px 0 0;
    font-size: 12px;
    color: var(--danger, #c0392b);
  }
  .list {
    flex: 1;
    margin: 6px 0 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }
  .list label {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 4px 2px;
  }
  .letter {
    margin-top: 6px;
    font-size: 12px;
    font-weight: 600;
    opacity: 0.6;
  }
  .code {
    margin-left: auto;
    font-size: 12px;
  }
  .muted {
    opacity: 0.7;
  }
  .disabled {
    opacity: 0.5;
  }
  .empty {
    padding: 16px 0;
    opacity: 0.8;
  }
  footer {
    margin-top: 12px;
  }
  .spacer {
    flex: 1;
  }
</style>
