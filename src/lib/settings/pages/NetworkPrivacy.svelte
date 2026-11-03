<script lang="ts">
  import { onMount } from "svelte";

  import Row from "$lib/components/settings/Row.svelte";
  import Section from "$lib/components/settings/Section.svelte";
  import SelectRow from "$lib/components/settings/SelectRow.svelte";
  import SwitchRow from "$lib/components/settings/SwitchRow.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { app, type ProxySettings, type Settings } from "$lib/ipc";
  import { patchSettings, store } from "$lib/stores/settings.svelte";

  import Page from "../Page.svelte";

  const SERVER_KINDS = ["Socks5Proxy", "HttpProxy", "HttpCachingProxy", "FtpCachingProxy"];
  const STRICT_KINDS = ["Socks5Proxy", "HttpProxy"];
  const DEFAULT_PROXY: ProxySettings = { enabled: false, kind: "NoProxy", host: "", port: 0, user: "" };

  const s = $derived(store.settings as Settings);
  // Proxy edits are a draft until applied, as in ZapZap.
  let draft = $state<ProxySettings>({ ...(store.settings as Settings).proxy });
  let strict = $state((store.settings as Settings).privacy.strict_proxy);
  let port = $state(String((store.settings as Settings).proxy.port || ""));
  let password = $state("");
  let passwordDirty = $state(false);
  let hasPassword = $state(false);
  let showAuth = $state(false);
  let error = $state("");
  let confirmRestore = $state(false);

  onMount(() => {
    app.proxyPasswordPresent().then((present) => {
      hasPassword = present;
      showAuth = present || !!draft.user;
    });
  });

  const needsServer = $derived(SERVER_KINDS.includes(draft.kind));
  const strictAvailable = $derived(draft.enabled && STRICT_KINDS.includes(draft.kind));
  const dirty = $derived(
    JSON.stringify({ ...draft, port: Number(port) || 0 }) !== JSON.stringify(s.proxy) ||
      strict !== s.privacy.strict_proxy ||
      passwordDirty,
  );
  const status = $derived.by(() => {
    const p = s.proxy;
    let text =
      p.kind === "NoProxy"
        ? t("No proxy configured")
        : p.kind === "DefaultProxy"
          ? t("System proxy")
          : `${p.kind} · ${p.host}:${p.port}`;
    if (!p.enabled && p.kind !== "NoProxy") text += ` · ${t("disabled")}`;
    return text;
  });

  function discard() {
    draft = { ...s.proxy };
    port = String(s.proxy.port || "");
    strict = s.privacy.strict_proxy;
    password = "";
    passwordDirty = false;
    error = "";
  }

  async function apply() {
    error = "";
    const portNumber = Number(port) || 0;
    if (draft.enabled && needsServer) {
      if (!draft.host.trim()) return (error = t("Enter a proxy server."));
      if (!Number.isInteger(portNumber) || portNumber < 1 || portNumber > 65535) {
        return (error = t("Enter a port between 1 and 65535."));
      }
    }
    try {
      if (passwordDirty) {
        await app.setProxyPassword(password);
        hasPassword = password !== "";
      }
      await patchSettings({
        proxy: { ...draft, host: draft.host.trim(), port: portNumber },
        privacy: { strict_proxy: strict && (strictAvailable || strict === s.privacy.strict_proxy) },
      });
      password = "";
      passwordDirty = false;
    } catch {
      error = t("Unable to apply the proxy settings. The previous proxy remains active.");
    }
  }

  async function restore() {
    confirmRestore = false;
    await app.setProxyPassword("");
    hasPassword = false;
    await patchSettings({ proxy: DEFAULT_PROXY });
    discard();
  }
</script>

<Page title={t("Privacy and Network")} description={t("Configure proxy, privacy protection, and network options.")}>
  <Section
    title={t("Proxy")}
    description={t("Route WhatsApp Web traffic through a proxy server. Existing connections may require reloading.")}
  >
    <SwitchRow
      title={t("Use proxy")}
      description={t("Route traffic through the proxy configured below.")}
      checked={draft.enabled}
      onchange={(enabled) => (draft.enabled = enabled)}
    />
    <SelectRow
      child
      disabled={!draft.enabled}
      title={t("Type")}
      description={t("Select the proxy mode.")}
      value={draft.kind}
      options={[
        { value: "NoProxy", label: t("No proxy") },
        { value: "DefaultProxy", label: t("System proxy") },
        { value: "Socks5Proxy", label: "SOCKS5" },
        { value: "HttpProxy", label: "HTTP" },
        { value: "HttpCachingProxy", label: t("HTTP caching") },
        { value: "FtpCachingProxy", label: t("FTP caching") },
      ]}
      onchange={(kind) => (draft.kind = kind)}
    />
    {#if needsServer}
      <Row child disabled={!draft.enabled} title={t("Server")} description={t("Proxy host name or IP address.")}>
        {#snippet control()}
          <input aria-label={t("Server")} placeholder="proxy.example.com" bind:value={draft.host} />
        {/snippet}
      </Row>
      <Row child disabled={!draft.enabled} title={t("Port")} description={t("Proxy port.")}>
        {#snippet control()}
          <input aria-label={t("Port")} placeholder="8080" inputmode="numeric" size="6" bind:value={port} />
        {/snippet}
      </Row>
      <Row
        child
        disabled={!draft.enabled}
        title={t("Authentication")}
        description={t("Show or hide optional proxy credentials.")}
      >
        {#snippet control()}
          <button class="btn" aria-expanded={showAuth} onclick={() => (showAuth = !showAuth)}>
            {showAuth ? "▴" : "▾"}
          </button>
        {/snippet}
      </Row>
      {#if showAuth}
        <Row child disabled={!draft.enabled} title={t("User")} description={t("Optional proxy user.")}>
          {#snippet control()}
            <input aria-label={t("User")} bind:value={draft.user} />
          {/snippet}
        </Row>
        <Row child disabled={!draft.enabled} title={t("Password")} description={t("Optional proxy password.")}>
          {#snippet control()}
            <input
              type="password"
              aria-label={t("Password")}
              placeholder={hasPassword && !passwordDirty ? "••••••••" : ""}
              bind:value={password}
              oninput={() => (passwordDirty = true)}
            />
          {/snippet}
        </Row>
        <p class="info-box warning auth">
          {t("The system web view cannot authenticate to proxies; credentials are kept but not used.")}
        </p>
      {/if}
    {/if}
    <SwitchRow
      child
      disabled={!strictAvailable && !strict}
      title={t("Strict proxy isolation")}
      description={t("Prevent direct network connections when an explicit proxy is configured.")}
      checked={strict}
      onchange={(v) => (strict = v)}
    />
    <Row child title={strictAvailable ? t("Disables WebRTC so it cannot bypass the proxy.") : t("Available only while an HTTP or SOCKS5 proxy is enabled.")} />
    <Row title={status} />
    <Row title={t("Restore proxy…")} description={t("Remove the global proxy settings and restore defaults.")}>
      {#snippet control()}
        <button class="btn danger" onclick={() => (confirmRestore = true)}>{t("Restore…")}</button>
      {/snippet}
    </Row>
  </Section>

  {#if dirty || error}
    <div class="draft" role="status">
      <span>{error || t("There are unapplied network changes.")}</span>
      <div>
        <button class="btn" onclick={discard}>{t("Discard")}</button>
        <button class="btn primary" onclick={apply}>{t("Apply changes")}</button>
      </div>
    </div>
  {/if}

  <Section title={t("Privacy")} description={t("Reduce the exposure of network information.")}>
    <SwitchRow
      title={t("WebRTC protection")}
      description={t("Blocks WebRTC APIs in page scripts. This is independent from strict proxy isolation.")}
      checked={s.privacy.webrtc_shield}
      onchange={(webrtc_shield) => patchSettings({ privacy: { webrtc_shield } })}
    />
  </Section>
</Page>

{#if confirmRestore}
  <ConfirmDialog
    title={t("Restore proxy settings?")}
    message={t("The global proxy settings will be removed and the default values will be restored.")}
    detail={t("Strict proxy isolation and WebRTC protection will not be changed.")}
    confirmLabel={t("Restore")}
    danger
    onconfirm={restore}
    oncancel={() => (confirmRestore = false)}
  />
{/if}

<style>
  .draft {
    position: sticky;
    bottom: 0;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: -12px 0 24px;
    padding: 10px 16px;
    border: 1px solid var(--accent);
    border-radius: 10px;
    background: var(--sidebar-bg);
  }
  .draft div {
    display: flex;
    gap: 6px;
  }
  .auth {
    margin: 0 16px 12px 36px;
  }
</style>
