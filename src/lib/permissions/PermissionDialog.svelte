<script lang="ts">
  // ZapZap's permission question; "No" is the default and is not remembered.
  import { invoke } from "@tauri-apps/api/core";

  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { PermissionPrompt } from "$lib/ipc";

  let { prompt, ondone }: { prompt: PermissionPrompt; ondone: () => void } = $props();

  const WHAT: Record<PermissionPrompt["feature"], string> = {
    microphone: "your microphone",
    camera: "your camera",
    camera_microphone: "your camera and microphone",
    location: "your location",
    screen_contents: "your screen contents",
    screen_contents_audio: "your screen contents and audio",
    mouse_lock: "mouse lock",
  };

  async function answer(allow: boolean) {
    await invoke("permission_answer", { id: prompt.id, allow });
    ondone();
  }
</script>

<ConfirmDialog
  title={t("Permission request")}
  message={t("WhatsApp Web is requesting access to {}.\n\nAllow?").replace("{}", t(WHAT[prompt.feature]))}
  confirmLabel={t("Yes")}
  cancelLabel={t("No")}
  onconfirm={() => answer(true)}
  oncancel={() => answer(false)}
/>
