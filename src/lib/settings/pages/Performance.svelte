<script lang="ts">
  import Row from "$lib/components/settings/Row.svelte";
  import Section from "$lib/components/settings/Section.svelte";
  import SelectRow from "$lib/components/settings/SelectRow.svelte";
  import SwitchRow from "$lib/components/settings/SwitchRow.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { PerformanceSettings, Settings } from "$lib/ipc";
  import { patchSettings, store } from "$lib/stores/settings.svelte";

  import Page from "../Page.svelte";

  type BoolKey = {
    [K in keyof PerformanceSettings]: PerformanceSettings[K] extends boolean ? K : never;
  }[keyof PerformanceSettings];

  // Chromium-only options are kept for ZapZap round trips but have no effect
  // on the system web views; they are shown disabled (see docs/parity.md).
  const SUPPORTED = new Set<string>([
    "disable_gpu",
    "software_rendering",
    "scroll_animator",
    "disable_animations",
    "ctrl_arrow_visual_navigation_fix",
  ]);
  const RENDERING: BoolKey[] = [
    "in_process_gpu",
    "disable_gpu",
    "auto_gpu_workaround",
    "disable_gpu_vsync",
    "software_rendering",
    "disable_gpu_memory_buffer_video_frames",
    "disable_zero_copy",
    "software_video_decoding",
    "force_gbm",
  ];
  const DEFAULTS: PerformanceSettings = {
    cache_type: "DiskHttpCache",
    cache_size_max: "0",
    js_memory_limit_index: 0,
    js_memory_limit_mb: "0",
    persistent_cookies: true,
    in_process_gpu: false,
    disable_gpu: false,
    auto_gpu_workaround: true,
    disable_gpu_vsync: false,
    software_rendering: false,
    disable_gpu_memory_buffer_video_frames: false,
    disable_zero_copy: false,
    software_video_decoding: false,
    force_gbm: false,
    disable_accessibility: false,
    single_process: false,
    process_per_site: true,
    js_predictable_gc_schedule: false,
    scroll_animator: false,
    background_throttling: true,
    disable_animations: false,
    disable_pinch: false,
    ctrl_arrow_visual_navigation_fix: true,
  };
  const COMPATIBILITY: Partial<PerformanceSettings> = {
    disable_gpu_memory_buffer_video_frames: true,
    disable_zero_copy: true,
    software_video_decoding: true,
  };

  const p = $derived((store.settings as Settings).performance);
  const set = (performance: Partial<PerformanceSettings>) => patchSettings({ performance });
  const unsupported = t("Not available with the system web view.");

  const matches = (preset: Partial<PerformanceSettings>) =>
    RENDERING.every((k) => p[k] === (preset[k] ?? DEFAULTS[k]));
  const profile = $derived(matches({}) ? "default" : matches(COMPATIBILITY) ? "compatibility" : "manual");

  function setProfile(value: string) {
    if (value === "manual") return;
    const preset = value === "compatibility" ? COMPATIBILITY : {};
    set(Object.fromEntries(RENDERING.map((k) => [k, preset[k] ?? DEFAULTS[k]])));
  }

  const GPU: { key: BoolKey; label: string; description: string }[] = [
    { key: "in_process_gpu", label: "In-process GPU", description: "May reduce memory usage, but can cause instability." },
    { key: "disable_gpu", label: "Disable GPU", description: "Recommended only for old GPUs or problematic drivers." },
    {
      key: "auto_gpu_workaround",
      label: "Automatic multi-GPU workaround",
      description: "Honors DRI_PRIME or adjusts GPU compositing for a headless secondary GPU.",
    },
    { key: "disable_gpu_vsync", label: "Disable GPU VSync", description: "May reduce latency, but can increase tearing." },
    { key: "software_rendering", label: "Force software rendering", description: "Use only for graphical issues." },
    {
      key: "disable_gpu_memory_buffer_video_frames",
      label: "Disable GPU memory buffer for video",
      description:
        "Avoid GPU-backed video frame buffers. May help with black screens, flickering and GPU crashes.",
    },
    {
      key: "disable_zero_copy",
      label: "Disable zero-copy",
      description: "Disable zero-copy GPU buffer transfers. May improve compatibility with some graphics drivers.",
    },
    {
      key: "software_video_decoding",
      label: "Use software video decoding",
      description:
        "Disables hardware video decoding to fix videos that appear white, black, flicker, or cause crashes. This may increase CPU and battery usage.",
    },
    { key: "force_gbm", label: "Force GBM", description: "Known NVIDIA typing lag fix." },
    {
      key: "disable_accessibility",
      label: "Disable accessibility bus",
      description: "Known typing lag or CPU spike fix in QtWebEngine.",
    },
  ];
  const WEB: { key: BoolKey; label: string; description: string }[] = [
    { key: "scroll_animator", label: "Smooth scrolling", description: "Disabling may reduce CPU usage." },
    {
      key: "background_throttling",
      label: "Background throttling",
      description: "Allows background tabs to reduce resource usage.",
    },
    { key: "disable_animations", label: "Disable animations", description: "May improve performance on slower machines." },
    {
      key: "disable_pinch",
      label: "Disable pinch-to-zoom",
      description: "Useful if accidental zooming occurs during scrolling.",
    },
    {
      key: "ctrl_arrow_visual_navigation_fix",
      label: "Fix Ctrl+Arrow cursor movement",
      description: "Use visual word navigation in editable fields to fix RTL text cursor movement.",
    },
  ];
  const describe = (key: string, description: string) =>
    SUPPORTED.has(key) ? t(description) : `${t(description)} ${unsupported}`;
</script>

<Page title={t("Performance experimental")} description={t("Tune cache, GPU, processes, and advanced behavior.")}>
  <p class="info-box warning">
    {t(
      "Change these settings only if you understand the trade-offs. Restart FastZap after changing rendering, process, or JavaScript memory options.",
    )}
  </p>
  <Section
    title={t("Rendering profile")}
    description={t("Choose a predefined rendering configuration or customize individual options.")}
  >
    <SelectRow
      title={t("Rendering profile")}
      value={profile}
      options={[
        { value: "default", label: t("Default") },
        { value: "compatibility", label: t("Compatibility") },
        { value: "manual", label: t("Manual") },
      ]}
      onchange={setProfile}
    />
  </Section>
  <Section title={t("Cache")} description={t("Control HTTP cache and persistent cookies.")}>
    <SelectRow
      disabled
      title={t("Cache type")}
      description={describe("cache_type", "Defines where the HTTP cache is stored.")}
      value={p.cache_type}
      options={["DiskHttpCache", "MemoryHttpCache", "NoCache"].map((v) => ({ value: v, label: v }))}
      onchange={(cache_type) => set({ cache_type })}
    />
    <SelectRow
      child
      disabled
      title={t("Maximum cache size")}
      description={t("0 MiB uses Chromium's automatic cache management.")}
      value={p.cache_size_max}
      options={["0", "128", "256", "512", "1024", "2047"].map((v) => ({ value: v, label: `${v} MiB` }))}
      onchange={(cache_size_max) => set({ cache_size_max })}
    />
    <SwitchRow
      disabled
      title={t("Persistent cookies")}
      description={describe("persistent_cookies", "Keeps cookies between restarts.")}
      checked={p.persistent_cookies}
      onchange={(persistent_cookies) => set({ persistent_cookies })}
    />
  </Section>
  <Section title={t("GPU and rendering")} description={t("Rendering options for problematic drivers or hardware.")}>
    {#each GPU as item (item.key)}
      <SwitchRow
        disabled={!SUPPORTED.has(item.key)}
        title={t(item.label)}
        description={describe(item.key, item.description)}
        checked={p[item.key]}
        onchange={(v) => set({ [item.key]: v })}
      />
    {/each}
  </Section>
  <Section title={t("Processes")} description={t("Chromium process model options.")}>
    <SwitchRow
      disabled
      title={t("Single process")}
      description={describe("single_process", "Experimental; reduces memory usage but may cause crashes.")}
      checked={p.single_process}
      onchange={(single_process) => set({ single_process })}
    />
    <SwitchRow
      disabled
      title={t("Process per site")}
      description={describe("process_per_site", "Safer and more stable, but may increase memory usage.")}
      checked={p.process_per_site}
      onchange={(process_per_site) => set({ process_per_site })}
    />
  </Section>
  <Section title={t("JavaScript memory")} description={t("V8 memory limit. Restart required.")}>
    <SelectRow
      disabled
      title={t("JavaScript memory limit")}
      description={describe("js_memory_limit_index", "Automatic is the safest option.")}
      value={p.js_memory_limit_index}
      options={[
        { value: 0, label: t("Automatic") },
        { value: 1, label: "256 MB" },
        { value: 2, label: "1024 MB" },
        { value: 3, label: "4096 MB" },
      ]}
      onchange={(i) => set({ js_memory_limit_index: i, js_memory_limit_mb: ["0", "256", "1024", "4096"][i] })}
    />
    <SwitchRow
      disabled
      title={t("JavaScript GC predictable schedule (experimental)")}
      description={describe("js_predictable_gc_schedule", "Enables predictable V8 garbage collection and disables GPU.")}
      checked={p.js_predictable_gc_schedule}
      onchange={(v) => set({ js_predictable_gc_schedule: v })}
    />
  </Section>
  <Section title={t("Web behavior")} description={t("Behavior options for rendered web content.")}>
    {#each WEB as item (item.key)}
      <SwitchRow
        disabled={!SUPPORTED.has(item.key)}
        title={t(item.label)}
        description={describe(item.key, item.description)}
        checked={p[item.key]}
        onchange={(v) => set({ [item.key]: v })}
      />
    {/each}
  </Section>
  <Section title={t("Restore defaults")} description={t("Return performance settings to safe defaults.")}>
    <Row
      title={t("Restore performance defaults")}
      description={t("Restores all performance settings to safe default values.")}
    >
      {#snippet control()}
        <button class="btn" onclick={() => set(DEFAULTS)}>{t("Restore")}</button>
      {/snippet}
    </Row>
  </Section>
</Page>
