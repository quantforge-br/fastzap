<script lang="ts">
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import Row from "$lib/components/settings/Row.svelte";
  import Section from "$lib/components/settings/Section.svelte";
  import SwitchRow from "$lib/components/settings/SwitchRow.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { app, reporting, type Settings } from "$lib/ipc";
  import ReportsDialog from "$lib/reporting/ReportsDialog.svelte";
  import { openReport } from "$lib/reporting/request.svelte";
  import { patchSettings, store } from "$lib/stores/settings.svelte";

  import Page from "../Page.svelte";

  // ZapZap's Debugging page ("Report a problem").
  const s = $derived(store.settings as Settings);
  let confirming = $state<"explain" | "clear" | "reset" | null>(null);
  let showReports = $state(false);
  let showPrivacy = $state(false);
  let notice = $state("");

  function setCrashPrompts(enabled: boolean) {
    if (enabled && !s.reporting.crash_prompts_explained) {
      confirming = "explain";
      return;
    }
    patchSettings({ reporting: { crash_prompts: enabled } });
  }

  async function answer(confirmed: boolean) {
    const kind = confirming;
    confirming = null;
    if (kind === "explain") {
      patchSettings({ reporting: { crash_prompts: confirmed, crash_prompts_explained: true } });
    } else if (confirmed && kind === "clear") {
      await reporting.clear();
      notice = t("Local reports deleted.");
    } else if (confirmed && kind === "reset") {
      await app.resetSettings();
      notice = t("Settings restored to their defaults.");
    }
  }
</script>

<Page title={t("Report a problem")} description={t("Prepare a sanitized report to publish with your GitHub account.")}>
  <Section
    title={t("Report a problem")}
    description={t("You will always review the complete report before copying it and opening GitHub.")}
  >
    <Row
      title={t("Something is not working?")}
      description={t("Describe the problem in simple steps. Avoid personal information.")}
    >
      {#snippet control()}
        <button class="btn primary" onclick={() => openReport()}>{t("Report a problem…")}</button>
      {/snippet}
    </Row>
    <SwitchRow
      title={t("Notify me when a failure occurs")}
      description={t(
        "FastZap will prepare a sanitized local report after a serious failure. It will never publish or transmit it automatically.",
      )}
      checked={s.reporting.crash_prompts}
      onchange={setCrashPrompts}
    />
    <Row
      title={t("Information and privacy")}
      description={t("See what reports may include and how publication on GitHub works.")}
    >
      {#snippet control()}
        <button class="btn" aria-expanded={showPrivacy} onclick={() => (showPrivacy = !showPrivacy)}>
          {t("View information")}
        </button>
      {/snippet}
    </Row>
    {#if showPrivacy}
      <p class="info-box">
        {t(
          "Reports may include the FastZap version, operating system, installation type, architecture, web view version, error details, and sanitized logs.",
        )}<br /><br />
        {t(
          "Never sent: messages, contacts, phone numbers, cookies, WhatsApp session data, conversation content, passwords, or authentication tokens.",
        )}<br /><br />
        {t(
          "FastZap does not send reports. After review, it copies the report and opens the official GitHub repository quantmind-br/fastzap. You need a GitHub account to publish the public issue.",
        )}
      </p>
    {/if}
    <Row
      title={t("Recent local reports")}
      description={t("Review reports kept on this device and their GitHub handoff status.")}
    >
      {#snippet control()}
        <button class="btn" onclick={() => (showReports = true)}>{t("View local reports")}</button>
      {/snippet}
    </Row>
  </Section>

  <Section
    title={t("Technical files")}
    description={t("Manage local files used to diagnose errors. These files are not sent automatically.")}
  >
    <Row title={t("Diagnostics folder")}>
      {#snippet control()}
        <button class="btn" onclick={() => reporting.openFolder()}>{t("Open")}</button>
      {/snippet}
    </Row>
    <Row title={t("Delete all logs and reports")} description={t("Remove every diagnostic file.")}>
      {#snippet control()}
        <button class="btn danger" onclick={() => (confirming = "clear")}>{t("Delete")}</button>
      {/snippet}
    </Row>
  </Section>

  <Section title={t("Maintenance")}>
    <Row title={t("Reset settings")} description={t("Restore every preference to its default value.")}>
      {#snippet control()}
        <button class="btn danger" onclick={() => (confirming = "reset")}>{t("Reset…")}</button>
      {/snippet}
    </Row>
  </Section>
  {#if notice}<p class="info-box" role="status">{notice}</p>{/if}
</Page>

{#if confirming === "explain"}
  <ConfirmDialog
    title={t("Notify me when a failure occurs?")}
    message={t(
      "FastZap can prepare a sanitized technical report locally after a serious failure. You will review it on the next startup.\n\nFastZap never transmits or publishes the report automatically.",
    )}
    confirmLabel={t("Enable")}
    onconfirm={() => answer(true)}
    oncancel={() => answer(false)}
  />
{:else if confirming === "clear"}
  <ConfirmDialog
    title={t("Delete all logs and reports")}
    message={t("Remove every diagnostic file?")}
    confirmLabel={t("Delete")}
    danger
    onconfirm={() => answer(true)}
    oncancel={() => answer(false)}
  />
{:else if confirming === "reset"}
  <ConfirmDialog
    title={t("Reset settings?")}
    message={t("All preferences return to their defaults. Accounts and WhatsApp sessions are kept.")}
    confirmLabel={t("Reset settings")}
    danger
    onconfirm={() => answer(true)}
    oncancel={() => answer(false)}
  />
{/if}

{#if showReports}
  <ReportsDialog onclose={() => (showReports = false)} />
{/if}
