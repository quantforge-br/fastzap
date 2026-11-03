<script lang="ts">
  import { untrack } from "svelte";

  import { t } from "$lib/i18n/index.svelte";
  import { reporting, type ReportView } from "$lib/ipc";

  import { CATEGORIES, categoryLabel, FREQUENCIES, frequencyLabel, previewText } from "./format";

  // ZapZap's ProblemReportDialog: a form, then a review of the sanitized
  // report. A saved report opens straight on the review page.
  let { review = null, onclose }: { review?: ReportView | null; onclose: () => void } = $props();

  const reviewOnly = untrack(() => review !== null);
  let page = $state<"form" | "review">(untrack(() => (review ? "review" : "form")));
  let view = $state<ReportView | null>(untrack(() => review));
  let category = $state<string>("feature_not_working");
  let description = $state("");
  let expected = $state("");
  let frequency = $state<string>("unknown");
  let includeSystem = $state(true);
  let includeError = $state(true);
  let includeLogs = $state(true);
  let warning = $state("");
  let showTechnical = $state(false);
  let busy = $state(false);

  async function toReview() {
    if (!description.trim()) {
      warning = t("Describe what happened before continuing.");
      return;
    }
    warning = "";
    view = await reporting.preview({
      category,
      description,
      expected,
      frequency,
      include_system: includeSystem,
      include_error: includeError,
      include_logs: includeLogs,
    });
    page = "review";
  }

  async function handoff() {
    if (!view || busy) return;
    busy = true;
    try {
      const saved = await reporting.save(view.id, view.document);
      await navigator.clipboard.writeText(saved.markdown).catch(() => {});
      if (await reporting.openGithub(saved.id)) {
        onclose();
      } else {
        warning = t(
          "Could not open GitHub. The report is still copied to the clipboard. Open the official FastZap repository and paste it into a new issue.",
        );
      }
    } catch (e) {
      warning = (e as { message?: string })?.message ?? String(e);
    } finally {
      busy = false;
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
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Report a problem")}>
    {#if page === "form"}
      <h2>{t("Report a problem")}</h2>
      <p>{t("Describe what happened. You will review everything before anything is copied or GitHub is opened.")}</p>
      <p class="muted">{t("Avoid phone numbers, messages, contact names, or other personal information.")}</p>
      <div class="form">
        <label>
          {t("What is the problem?")}
          <select bind:value={category}>
            {#each CATEGORIES as id (id)}<option value={id}>{categoryLabel(id)}</option>{/each}
          </select>
        </label>
        <label>
          {t("Describe what happened")}
          <!-- svelte-ignore a11y_autofocus -->
          <textarea rows="5" autofocus bind:value={description}></textarea>
        </label>
        <label>
          {t("What did you expect to happen?")}
          <textarea rows="3" bind:value={expected}></textarea>
        </label>
        <label>
          {t("Can you repeat the problem?")}
          <select bind:value={frequency}>
            {#each FREQUENCIES as id (id)}<option value={id}>{frequencyLabel(id)}</option>{/each}
          </select>
        </label>
        <label class="check"><input type="checkbox" bind:checked={includeSystem} /> {t("Include system technical information")}</label>
        <label class="check"><input type="checkbox" bind:checked={includeError} /> {t("Include error technical information, when available")}</label>
        <label class="check"><input type="checkbox" bind:checked={includeLogs} /> {t("Include sanitized logs, when available")}</label>
      </div>
      {#if warning}<p class="warning" role="alert">{warning}</p>{/if}
      <footer>
        <button class="btn" onclick={onclose}>{t("Cancel")}</button>
        <button class="btn primary" onclick={toReview}>{t("Review report")}</button>
      </footer>
    {:else if view}
      <h2>{t("Review your report")}</h2>
      <p class="muted">
        {t("The complete report will be copied to the clipboard. After GitHub opens, paste it into the issue description.")}
      </p>
      <pre class="preview">{previewText(view)}</pre>
      <button class="link" aria-expanded={showTechnical} onclick={() => (showTechnical = !showTechnical)}>
        {showTechnical ? t("Hide technical information") : t("Show technical information")}
      </button>
      {#if showTechnical}
        <textarea class="markdown" readonly rows="10" aria-label={t("Complete report to copy to GitHub")}
          >{view.markdown}</textarea
        >
      {/if}
      {#if warning}<p class="warning" role="alert">{warning}</p>{/if}
      <footer>
        {#if reviewOnly}
          <button class="btn" onclick={onclose}>{t("Close")}</button>
        {:else}
          <button class="btn" onclick={() => (page = "form")}>{t("Back and edit")}</button>
        {/if}
        <button
          class="btn primary"
          disabled={busy}
          title={t("After GitHub opens, paste the copied report into the issue description.")}
          aria-label={t("Copy report to the clipboard and open GitHub")}
          onclick={handoff}>{t("Copy and open GitHub")}</button
        >
      </footer>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 40%);
  }
  .dialog {
    display: flex;
    flex-direction: column;
    width: min(720px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    padding: 16px 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 8px 32px rgb(0 0 0 / 30%);
    box-sizing: border-box;
    overflow-y: auto;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 17px;
  }
  p {
    margin: 0 0 8px;
  }
  .muted {
    opacity: 0.7;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .form label {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .form label.check {
    flex-direction: row;
    gap: 8px;
    align-items: center;
  }
  textarea {
    resize: vertical;
    font: inherit;
  }
  .preview,
  .markdown {
    margin: 0;
    padding: 10px;
    border: 1px solid var(--csr-mid);
    border-radius: 6px;
    font-family: monospace;
    font-size: 12px;
    white-space: pre-wrap;
  }
  .preview {
    max-height: 340px;
    overflow-y: auto;
  }
  .markdown {
    margin-top: 6px;
  }
  .link {
    align-self: flex-start;
    margin: 8px 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent, inherit);
    text-decoration: underline;
    cursor: pointer;
  }
  .warning {
    margin-top: 8px;
    color: var(--danger, #c0392b);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
</style>
