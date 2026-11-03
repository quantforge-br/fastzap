<script lang="ts">
  // ZapZap's Advanced Customizations page. File changes apply at once; the
  // category switches and "inherit" wait for Save.
  import { Menu, MenuItem } from "@tauri-apps/api/menu";
  import { onMount } from "svelte";

  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import Row from "$lib/components/settings/Row.svelte";
  import Section from "$lib/components/settings/Section.svelte";
  import Switch from "$lib/components/settings/Switch.svelte";
  import FileEditor from "$lib/customizations/FileEditor.svelte";
  import { accountKey, custom, type AssetKind, type CustomFile, type Scope } from "$lib/customizations/api";
  import { t } from "$lib/i18n/index.svelte";
  import { accounts as accountsApi, layout as layoutApi, type Account, type Settings } from "$lib/ipc";
  import { patchSettings, store } from "$lib/stores/settings.svelte";

  import Page from "../Page.svelte";

  let current = $state<Account | null>(null);
  let scope = $state<Scope>(null);
  let kind = $state<AssetKind>("css");
  let files = $state<CustomFile[]>([]);
  let selected = $state<string | null>(null);
  let preview = $state<string | null>(null);
  let feedback = $state("");
  let draft = $state({ css: false, js: false, inherit: true });
  let dirty = $state(false);
  let editor = $state<{ oldName: string | null; name: string; content: string } | null>(null);
  let urlDialog = $state<{ url: string; name: string; error: string } | null>(null);
  let deleting = $state<string | null>(null);

  const c = $derived((store.settings as Settings).customizations);
  const locked = $derived(scope !== null && draft.inherit);

  function stored(s: Scope) {
    const entry = s === null ? null : c.accounts[accountKey(s)];
    const toggles = s === null ? c.global : entry;
    return {
      css: toggles?.css.enabled ?? false,
      js: toggles?.js.enabled ?? false,
      inherit: entry?.inherit ?? true,
    };
  }

  function say(message: string) {
    feedback = message;
  }

  async function refresh() {
    files = await custom.list(scope, kind);
    if (selected && !files.some((f) => f.name === selected)) selected = null;
    preview = kind === "css" && selected ? await custom.preview(scope, selected) : null;
  }

  onMount(async () => {
    const [list, layout] = await Promise.all([accountsApi.list(), layoutApi.get()]);
    current = list.find((a) => a.id === layout.active) ?? null;
    draft = stored(scope);
    refresh();
  });

  async function save() {
    const patch =
      scope === null
        ? { global: { css: { enabled: draft.css }, js: { enabled: draft.js } } }
        : {
            accounts: {
              [accountKey(scope)]: draft.inherit
                ? { inherit: true }
                : { inherit: false, css: { enabled: draft.css }, js: { enabled: draft.js } },
            },
          };
    await patchSettings({ customizations: patch });
    dirty = false;
    say(t("Customizations saved. Reload pages to apply changes."));
  }

  async function changeScope(next: Scope) {
    if (dirty) {
      await save();
      say(t("Changes to the previous scope were saved."));
    }
    scope = next;
    draft = stored(next);
    selected = null;
    refresh();
  }

  function setDraft(change: Partial<typeof draft>) {
    draft = { ...draft, ...change };
    dirty = true;
  }

  function discard() {
    draft = stored(scope);
    dirty = false;
    say(t("Unsaved changes discarded."));
  }

  async function reload() {
    await custom.reload(scope);
    say(t("All pages reloaded."));
  }

  async function toggleFile(file: CustomFile, enabled: boolean) {
    await custom.setFileEnabled(scope, kind, file.name, enabled);
    const label = kind === "css" ? (enabled ? "CSS file enabled: {}" : "CSS file disabled: {}") : enabled ? "JS file enabled: {}" : "JS file disabled: {}";
    say(`${t(label).replace("{}", file.name)}. ${t("Save and reload to apply changes.")}`);
    refresh();
  }

  async function openEditor(name: string | null) {
    if (name === null) {
      editor = { oldName: null, name: kind === "css" ? "custom.css" : "custom.js", content: "" };
      return;
    }
    try {
      editor = { oldName: name, name, content: await custom.read(scope, kind, name) };
    } catch {
      say(t("Could not open file for editing."));
    }
  }

  async function saveEditor(name: string, content: string): Promise<string | null> {
    if (!name.trim()) return t("File name cannot be empty.");
    const creating = editor?.oldName === null;
    try {
      const saved = await custom.write(scope, kind, editor?.oldName ?? null, name, content);
      editor = null;
      say(t(creating ? "File created: {}" : "File saved: {}").replace("{}", saved));
      selected = saved;
      refresh();
      return null;
    } catch (e) {
      return String(e) || t(creating ? "Could not create file." : "Could not save file.");
    }
  }

  async function importFile() {
    try {
      const name = await custom.importFile(scope, kind);
      if (name) {
        say(t("File imported successfully."));
        refresh();
      }
    } catch (e) {
      say(`${t("Could not import file.")} ${e}`);
    }
  }

  async function importUrl() {
    if (!urlDialog) return;
    if (!urlDialog.url.trim()) {
      urlDialog.error = kind === "css" ? t("Enter a CSS HTTPS URL.") : t("Enter a JavaScript HTTPS URL.");
      return;
    }
    try {
      const result = await custom.importUrl(scope, kind, urlDialog.url, urlDialog.name || null);
      urlDialog = null;
      say(
        kind === "css" && !result.preview
          ? t("File imported: {}. No preview found, upload one manually.").replace("{}", result.name)
          : t("File imported: {}").replace("{}", result.name),
      );
      selected = result.name;
      refresh();
    } catch (e) {
      if (urlDialog) urlDialog.error = String(e);
    }
  }

  async function fileMenu(file: CustomFile) {
    const items = [
      await MenuItem.new({ text: t("Edit"), action: () => openEditor(file.name) }),
      await MenuItem.new({
        text: t("Duplicate"),
        action: async () => {
          try {
            say(t("File duplicated: {}").replace("{}", await custom.duplicate(scope, kind, file.name)));
            refresh();
          } catch {
            say(t("Could not duplicate file."));
          }
        },
      }),
      await MenuItem.new({ text: t("Open location"), action: () => custom.openFolder(scope, kind) }),
      await MenuItem.new({ text: t("Delete"), action: () => (deleting = file.name) }),
    ];
    await (await Menu.new({ items })).popup();
  }

  async function confirmDelete() {
    const name = deleting;
    deleting = null;
    if (!name) return;
    try {
      await custom.remove(scope, kind, name);
      say(t("File deleted."));
      refresh();
    } catch {
      say(t("Could not delete file."));
    }
  }

  async function select(name: string) {
    selected = name;
    preview = kind === "css" ? await custom.preview(scope, name) : null;
  }

  async function uploadPreview() {
    if (!selected) return;
    try {
      if (await custom.pickPreview(scope, selected)) {
        say(t("Preview image updated."));
        refresh();
      }
    } catch {
      say(t("Could not set preview image."));
    }
  }
</script>

<svelte:window
  onkeydowncapture={(e) => {
    if (urlDialog && e.key === "Escape") {
      e.preventDefault();
      urlDialog = null;
    }
  }}
/>

<Page title={t("Advanced customizations")} description={t("Add custom CSS and JavaScript to WhatsApp Web.")}>
  <Section
    title={t("Scope")}
    description={t("Choose whether customizations apply to all accounts or only to the current account.")}
  >
    <Row title={t("Apply to")} description={t("Account customizations can inherit or extend the global configuration.")}>
      <p class="muted small">
        {current
          ? t("Current account: {} ({})")
              .replace("{}", current.name || t("Unnamed account"))
              .replace("{}", String(current.id))
          : t("Current account: unavailable")}
      </p>
      {#snippet control()}
        <select value={scope === null ? "global" : "account"} onchange={(e) => changeScope(e.currentTarget.value === "account" && current ? current.id : null)}>
          <option value="global">{t("All accounts")}</option>
          <option value="account" disabled={!current}>{t("This account")}</option>
        </select>
      {/snippet}
    </Row>
    {#if scope !== null}
      <Row title={t("Inherit global settings")} description={t("Use only the global customizations for this account.")}>
        {#snippet control()}
          <Switch checked={draft.inherit} label={t("Inherit global settings")} onchange={(v) => setDraft({ inherit: v })} />
        {/snippet}
      </Row>
    {/if}
  </Section>

  <Section title={t("Customizations")} description={t("Manage custom files for the selected scope.")}>
    <div class="tabs" role="tablist">
      {#each ["css", "js"] as const as k (k)}
        <button
          role="tab"
          aria-selected={kind === k}
          disabled={locked}
          onclick={() => {
            kind = k;
            selected = null;
            refresh();
          }}>{k === "css" ? t("CSS") : t("JavaScript")}</button
        >
      {/each}
    </div>
    <div class="body" class:locked>
      <Row
        title={kind === "css" ? t("Custom CSS") : t("Custom JavaScript")}
        description={kind === "css"
          ? t("Enables or disables all CSS files in this scope.")
          : t("Enables or disables all JavaScript files in this scope.")}
      >
        {#snippet control()}
          <Switch
            checked={draft[kind]}
            label={kind === "css" ? t("Custom CSS") : t("Custom JavaScript")}
            disabled={locked}
            onchange={(v) => setDraft({ [kind]: v })}
          />
        {/snippet}
      </Row>
      {#if kind === "js"}
        <p class="info-box warning">
          {t("⚠ Custom scripts have full access to page content. Use code only from trusted sources.")}
        </p>
      {/if}
      <div class="toolbar">
        <button class="btn" disabled={locked} onclick={() => openEditor(null)}>{t("+ Create")}</button>
        <button class="btn" disabled={locked} onclick={importFile}>{t("Import file")}</button>
        <button class="btn" disabled={locked} onclick={() => (urlDialog = { url: "", name: "", error: "" })}>
          {t("Import URL")}
        </button>
        <button class="btn" onclick={() => custom.openFolder(scope, kind)}>{t("Open folder")}</button>
      </div>
      <div class="files">
        {#if files.length === 0}
          <p class="muted empty">
            {kind === "css" ? t("No CSS files") : t("No JavaScript files")} ·
            {kind === "css" ? t("Create or import a style to get started.") : t("Create or import a script to get started.")}
          </p>
        {:else}
          <ul>
            {#each files as file (file.name)}
              <li class:selected={selected === file.name}>
                <input
                  type="checkbox"
                  checked={file.enabled}
                  disabled={locked}
                  aria-label={file.name}
                  onchange={(e) => toggleFile(file, e.currentTarget.checked)}
                />
                <button class="name" ondblclick={() => openEditor(file.name)} onclick={() => select(file.name)}>
                  {file.name}
                </button>
                <button class="btn" title={t("File actions")} disabled={locked} onclick={() => fileMenu(file)}>⋯</button>
              </li>
            {/each}
          </ul>
        {/if}
        {#if kind === "css"}
          <aside class="preview">
            <strong>{t("Preview")}</strong>
            {#if preview}
              <img src={preview} alt={t("Preview")} />
            {:else}
              <p class="muted small">
                {selected ? t("No preview available.") : t("No preview available. Select a CSS file to view an associated screenshot.")}
              </p>
            {/if}
            {#if selected}
              <button class="btn" disabled={locked} onclick={uploadPreview}>
                {preview ? t("Replace preview image") : t("Upload preview image")}
              </button>
            {/if}
          </aside>
        {/if}
      </div>
    </div>
  </Section>

  <div class="footer">
    {#if dirty}<span class="unsaved">{t("● There are unsaved changes.")}</span>{/if}
    {#if feedback}<span class="muted small feedback">{feedback}</span>{/if}
    <button class="btn" disabled={!dirty} onclick={discard}>{t("Discard")}</button>
    <button class="btn" onclick={reload}>{t("Reload page")}</button>
    <button class="btn" onclick={async () => (await save(), reload())}>{t("Save and reload")}</button>
    <button class="btn primary" disabled={!dirty} onclick={save}>{t("Save")}</button>
  </div>
</Page>

{#if editor}
  <FileEditor
    {kind}
    title={editor.oldName === null
      ? t("Create file")
      : t(kind === "css" ? "Edit CSS: {}" : "Edit JavaScript: {}").replace("{}", editor.oldName)}
    name={editor.name}
    content={editor.content}
    onsave={saveEditor}
    oncancel={() => (editor = null)}
  />
{/if}

{#if urlDialog}
  <div class="backdrop">
    <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Import from URL")}>
      <h2>{t("Import from URL")}</h2>
      <label class="field">
        <span>{t("URL")}</span>
        <input
          bind:value={urlDialog.url}
          placeholder={kind === "css" ? "https://example.com/theme.user.css" : "https://example.com/script.js"}
        />
      </label>
      <label class="field">
        <span>{t("File name")}</span>
        <input bind:value={urlDialog.name} placeholder={t("Optional file name (e.g. 10-custom.css)")} />
      </label>
      {#if urlDialog.error}<p class="info-box warning">{urlDialog.error}</p>{/if}
      <div class="actions">
        <button class="btn" onclick={() => (urlDialog = null)}>{t("Cancel")}</button>
        <button class="btn primary" onclick={importUrl}>{t("OK")}</button>
      </div>
    </div>
  </div>
{/if}

{#if deleting}
  <ConfirmDialog
    title={t("Confirm deletion")}
    message={t("Delete selected {} file: {}?").replace("{}", kind === "css" ? "CSS" : "JS").replace("{}", deleting)}
    confirmLabel={t("Yes")}
    cancelLabel={t("No")}
    danger
    onconfirm={confirmDelete}
    oncancel={() => {
      deleting = null;
      say(t("Deletion cancelled."));
    }}
  />
{/if}

<style>
  .small {
    margin: 4px 0 0;
    font-size: 12px;
  }
  .tabs {
    display: flex;
    border-bottom: 1px solid var(--csr-mid);
  }
  .tabs button {
    padding: 8px 16px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .tabs button[aria-selected="true"] {
    border-bottom-color: var(--accent);
  }
  .locked {
    opacity: 0.6;
  }
  .info-box {
    margin: 0 16px 8px;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 8px 16px;
  }
  .files {
    display: flex;
    gap: 12px;
    padding: 0 16px 16px;
  }
  .empty {
    flex: 1;
    font-size: 13px;
  }
  ul {
    flex: 1;
    min-width: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px;
    border-radius: 6px;
  }
  li.selected {
    background: var(--csr-hover);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }
  .preview {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 220px;
  }
  .preview img {
    max-width: 100%;
    border-radius: 6px;
  }
  .footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }
  .unsaved {
    color: var(--accent);
    font-size: 12px;
  }
  .feedback {
    margin: 0 auto 0 0;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 45;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 40%);
  }
  .dialog {
    width: min(480px, calc(100vw - 32px));
    padding: 16px 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
  }
  .dialog h2 {
    margin: 0;
    font-size: 17px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 10px;
    font-size: 13px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
</style>
