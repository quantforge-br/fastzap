<script lang="ts">
  import { onMount } from "svelte";

  import Row from "$lib/components/settings/Row.svelte";
  import Section from "$lib/components/settings/Section.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { app, type AppInfo } from "$lib/ipc";

  import Page from "../Page.svelte";

  const REPOSITORY = "https://github.com/quantmind-br/fastzap";
  let info = $state<AppInfo | null>(null);
  let showDetails = $state(false);
  let copied = $state(false);
  let notice = $state("");

  onMount(() => {
    app.info().then((i) => (info = i));
  });

  function details(i: AppInfo): [string, string][] {
    return [
      [t("Version"), i.version],
      [t("Web view"), i.webview_version],
      [t("System"), `${i.os} ${i.arch}`],
      [t("Session"), [i.desktop, i.session_type].filter(Boolean).join(" / ")],
      [t("Packaging"), i.packaging],
      [t("Channel"), i.channel],
      [t("Provider"), i.provider],
      [t("Repository"), i.repository],
    ].filter(([, v]) => v) as [string, string][];
  }

  async function copyInfo() {
    if (!info) return;
    const text = [`FastZap ${info.version}`, ...details(info).slice(1).map(([k, v]) => `${k}: ${v}`)].join("\n");
    await navigator.clipboard.writeText(text);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }
</script>

<Page title={t("About")} description={t("Information about FastZap, the project, and support.")}>
  <div class="identity">
    <img src="/favicon.png" alt="" />
    <div>
      <h2>FastZap</h2>
      <p class="muted">{t("WhatsApp Web for Linux")}</p>
      {#if info}<p class="muted">{t("Version {version}", { version: info.version })}</p>{/if}
    </div>
  </div>

  <Section title={t("Project and support")}>
    <Row title={t("Project page")} description={t("Official website, downloads, and documentation.")}>
      {#snippet control()}
        <a class="btn" href={REPOSITORY} target="_blank" rel="noreferrer">↗</a>
      {/snippet}
    </Row>
    <Row title={t("Report a problem")} description={t("Report bugs or suggest improvements on GitHub.")}>
      {#snippet control()}
        <a class="btn" href={`${REPOSITORY}/issues`} target="_blank" rel="noreferrer">↗</a>
      {/snippet}
    </Row>
    <Row title={t("Support FastZap")} description={t("Learn how to contribute to the project's development.")}>
      {#snippet control()}
        <button class="btn" onclick={() => app.action("donate")}>♥</button>
      {/snippet}
    </Row>
  </Section>

  <Section title={t("Information")}>
    <Row title={t("Technical details")}>
      {#if showDetails && info}
        <dl>
          {#each details(info) as [key, value] (key)}
            <dt>{key}</dt>
            <dd>{value}</dd>
          {/each}
        </dl>
      {/if}
      {#snippet control()}
        <button class="btn" aria-expanded={showDetails} onclick={() => (showDetails = !showDetails)}>
          {showDetails ? "▴" : "▾"}
        </button>
      {/snippet}
    </Row>
    <Row title={t("License")} description="GPL-3.0-or-later">
      {#snippet control()}
        <button
          class="btn"
          onclick={() =>
            (notice = t(
              "FastZap is free software licensed under GNU General Public License v3.0 (GPL-3.0-or-later).",
            ))}>ⓘ</button
        >
      {/snippet}
    </Row>
    <Row title={t("Credits and contributors")}>
      {#snippet control()}
        <button
          class="btn"
          onclick={() =>
            (notice = t(
              "FastZap is a Rust and Tauri rewrite of ZapZap, created and maintained by Rafael Tosta. Thanks to everyone who contributes translations, code, testing, and feedback.",
            ))}>ⓘ</button
        >
      {/snippet}
    </Row>
  </Section>
  {#if notice}<p class="info-box">{notice}</p>{/if}

  <div class="footer">
    <button class="btn" onclick={copyInfo}>{copied ? t("Information copied") : t("Copy system information")}</button>
    <p class="muted">{t("FastZap is an independent project and is not affiliated with WhatsApp or Meta.")}</p>
  </div>
</Page>

<style>
  .identity {
    display: flex;
    gap: 16px;
    align-items: center;
    margin-bottom: 24px;
  }
  .identity img {
    width: 64px;
    height: 64px;
  }
  .identity h2 {
    margin: 0;
  }
  .identity p {
    margin: 2px 0 0;
  }
  a.btn {
    text-decoration: none;
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 12px;
    margin: 8px 0 0;
    font-size: 12px;
  }
  dt {
    opacity: 0.7;
  }
  dd {
    margin: 0;
    word-break: break-all;
  }
  .footer {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }
</style>
