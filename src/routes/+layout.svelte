<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";

  import { setLocale } from "$lib/i18n/index.svelte";
  import { app, events, on, type Settings } from "$lib/ipc";

  let { children } = $props();

  onMount(() => {
    const refresh = () => app.locale().then(setLocale);
    refresh();
    let language: string | null = null;
    const unlisten = on<Settings>(events.settingsChanged, (s) => {
      if (s.system.interface_language !== language) {
        language = s.system.interface_language;
        refresh();
      }
    });
    return () => unlisten.then((f) => f());
  });
</script>

{@render children()}
