<script lang="ts">
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { spellcheck, type SpellState } from "$lib/ipc";

  import { dictionaryLabel, matches, sizeLabel } from "./format";

  // ZapZap's dictionary manager over the pinned Hunspell catalog.
  let {
    spell,
    onclose,
    onlanguages,
  }: {
    spell: SpellState;
    onclose: () => void;
    onlanguages: () => void;
  } = $props();

  type Filter = "all" | "installed" | "available";
  let query = $state("");
  let filter = $state<Filter>("all");
  let message = $state("");
  let removing = $state<{ tag: string; last: boolean } | null>(null);

  const rows = $derived.by(() => {
    const installed = new Map(spell.installed.map((d) => [d.tag, d.managed]));
    const sizes = new Map(spell.available.map((d) => [d.tag, d.size]));
    const tags = new Set([...installed.keys(), ...sizes.keys()]);
    return [...tags]
      .map((tag) => {
        const managed = installed.get(tag);
        const status =
          managed === undefined
            ? "available"
            : spell.languages.includes(tag)
              ? "active"
              : managed
                ? "installed"
                : "system";
        return { tag, label: dictionaryLabel(tag), managed, status, size: sizes.get(tag) };
      })
      .sort((a, b) => a.label.localeCompare(b.label) || a.tag.localeCompare(b.tag));
  });
  const visible = $derived(
    rows.filter(
      (r) =>
        (filter === "all" || (filter === "available") === (r.status === "available")) &&
        matches(query, r.label, r.tag),
    ),
  );
  const statusLabel: Record<string, () => string> = {
    active: () => t("Active"),
    installed: () => t("Installed"),
    system: () => t("System"),
    available: () => t("Available"),
  };

  const errorText = (e: unknown) => t("Dictionary operation failed: {msg}").replace("{msg}", (e as { message?: string })?.message ?? String(e));

  async function install(tag: string) {
    message = t("Downloading {code}…").replace("{code}", tag);
    try {
      await spellcheck.install(tag);
      message = t("Dictionary {code} is ready.").replace("{code}", tag);
    } catch (e) {
      message = errorText(e);
    }
  }

  function askRemove(tag: string) {
    const last = spell.enabled && spell.languages.length === 1 && spell.languages[0] === tag;
    removing = { tag, last };
  }

  async function remove() {
    const request = removing;
    removing = null;
    if (!request) return;
    try {
      await spellcheck.remove(request.tag, request.last);
      message = "";
    } catch (e) {
      message = errorText(e);
    }
  }

  async function importFile() {
    try {
      const tag = await spellcheck.import();
      if (tag) message = t("Dictionary {code} is ready.").replace("{code}", tag);
    } catch (e) {
      message = errorText(e);
    }
  }
</script>

<svelte:window
  onkeydowncapture={(e) => {
    if (e.key !== "Escape" || removing) return;
    e.preventDefault();
    if (query) query = "";
    else onclose();
  }}
/>

<div class="backdrop">
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Manage dictionaries")}>
    <header>
      <h2>{t("Manage dictionaries")}</h2>
      <button class="close" title={t("Close")} aria-label={t("Close")} onclick={onclose}>✕</button>
    </header>
    <p class="muted">
      {t("Install Hunspell dictionaries for spell checking. Dictionaries found on the system are used too.")}
    </p>
    <p class="muted small">
      {t("Catalog {revision}: {count} dictionaries")
        .replace("{revision}", spell.catalog_revision)
        .replace("{count}", String(spell.available.length))}
    </p>

    <div class="tools">
      <!-- svelte-ignore a11y_autofocus -->
      <input
        type="search"
        autofocus
        placeholder={t("Search language or code")}
        aria-label={t("Search language or code")}
        bind:value={query}
      />
      <select bind:value={filter} aria-label={t("Filter")}>
        <option value="all">{t("All")}</option>
        <option value="installed">{t("Installed")}</option>
        <option value="available">{t("Available")}</option>
      </select>
    </div>
    <p class="muted small">
      {spell.languages.length
        ? t("Active: {languages}").replace("{languages}", spell.languages.map(dictionaryLabel).join(", "))
        : t("No active dictionary")}
    </p>

    <table>
      <thead>
        <tr><th>{t("Language")}</th><th>{t("Code")}</th><th>{t("Status")}</th><th></th></tr>
      </thead>
      <tbody>
        {#each visible as row (row.tag)}
          {@const busy = spell.busy.includes(row.tag)}
          <tr>
            <td>{row.label}</td>
            <td class="muted">{row.tag}</td>
            <td>{busy ? t("Downloading…") : statusLabel[row.status]()}</td>
            <td class="actions">
              {#if row.status === "available"}
                <button class="btn" disabled={busy} onclick={() => install(row.tag)}>
                  {t("Install")}{#if row.size}&nbsp;<span class="muted">({sizeLabel(row.size)})</span>{/if}
                </button>
              {:else if row.managed}
                <button class="btn" onclick={() => askRemove(row.tag)}>{t("Remove")}</button>
              {/if}
            </td>
          </tr>
        {:else}
          <tr><td colspan="4" class="empty">{t("No dictionary matches the search.")}</td></tr>
        {/each}
      </tbody>
    </table>

    <p class="status" role="status">{message}</p>
    <footer>
      <button class="btn" onclick={importFile}>{t("Import files…")}</button>
      <button class="btn" onclick={() => spellcheck.openFolder()}>{t("Open folder")}</button>
      <button class="btn" disabled={!spell.installed.length} onclick={onlanguages}>{t("Active languages…")}</button>
      <span class="spacer"></span>
      <button class="btn primary" onclick={onclose}>{t("Close")}</button>
    </footer>
  </div>
</div>

{#if removing}
  <ConfirmDialog
    title={removing.last ? t("Disable spell checker and remove") : t("Remove dictionary")}
    message={(removing.last
      ? t("This is the last active dictionary. Disable the spell checker and remove {code}?")
      : t("Remove the dictionary {code}?")
    ).replace("{code}", removing.tag)}
    confirmLabel={t("Remove")}
    danger
    onconfirm={remove}
    oncancel={() => (removing = null)}
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
    display: flex;
    flex-direction: column;
    width: min(760px, calc(100vw - 32px));
    height: min(720px, calc(100vh - 32px));
    padding: 16px 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
    box-sizing: border-box;
  }
  header,
  footer,
  .tools {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  header {
    justify-content: space-between;
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
  .tools input {
    flex: 1;
  }
  table {
    display: block;
    flex: 1;
    margin-top: 6px;
    overflow-y: auto;
    border-collapse: collapse;
  }
  thead,
  tbody,
  tr {
    display: table;
    width: 100%;
    table-layout: fixed;
  }
  th {
    font-size: 12px;
    text-align: left;
    opacity: 0.7;
  }
  td,
  th {
    padding: 4px 6px;
  }
  .actions {
    text-align: right;
  }
  .muted {
    opacity: 0.7;
  }
  .small {
    font-size: 12px;
  }
  .status {
    min-height: 1.2em;
  }
  .empty {
    padding: 16px 0;
    text-align: center;
    opacity: 0.8;
  }
  footer {
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .spacer {
    flex: 1;
  }
</style>
