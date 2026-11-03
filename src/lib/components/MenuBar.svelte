<script lang="ts">
  // Main window menu bar (ZapZap's File/View/Chat/Users/Help menus).
  import { t } from "$lib/i18n/index.svelte";
  import { CheckMenuItem, Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
  import { LogicalPosition } from "@tauri-apps/api/dpi";

  import { accounts as accountsApi, app, type Account } from "$lib/ipc";

  let {
    accounts,
    sidebar,
    scale,
    onaction,
  }: { accounts: Account[]; sidebar: boolean; scale: number; onaction: (action: string) => void } = $props();

  interface Item {
    label: string;
    shortcut?: string;
    run: () => void;
    checked?: boolean;
    /** Account names are shown as is. */
    translate?: boolean;
  }
  type Entry = Item | "separator";

  const act = (action: string) => () => onaction(action);
  const backend = (action: string) => () => app.action(action);

  const menus = $derived<{ title: string; items: Entry[] }[]>([
    {
      title: "File",
      items: [
        { label: "Settings", shortcut: "Ctrl+P", run: act("settings") },
        "separator",
        { label: "Hide", shortcut: "Ctrl+W", run: backend("hide") },
        { label: "Quit", shortcut: "Ctrl+Q", run: backend("quit") },
      ],
    },
    {
      title: "View",
      items: [
        { label: "Grid View", shortcut: "Ctrl+G", run: backend("toggle_grid") },
        { label: "Show monitoring panel", run: backend("show_panel") },
        { label: "Downloads", shortcut: "Ctrl+J", run: act("downloads") },
        { label: "Open DevTools", shortcut: "Ctrl+Shift+I", run: backend("devtools") },
        "separator",
        { label: "Show sidebar", run: act("toggle_sidebar"), checked: sidebar },
        { label: "Toggle full screen", shortcut: "F11", run: backend("fullscreen") },
        "separator",
        { label: "Automatic theme", shortcut: "Ctrl+Alt+1", run: backend("theme_auto") },
        { label: "Light theme", shortcut: "Ctrl+Alt+2", run: backend("theme_light") },
        { label: "Dark theme", shortcut: "Ctrl+Alt+3", run: backend("theme_dark") },
        "separator",
        { label: "Reset zoom", shortcut: "Ctrl+0", run: backend("zoom_reset") },
        { label: "Zoom in", shortcut: "Ctrl++", run: backend("zoom_in") },
        { label: "Zoom out", shortcut: "Ctrl+-", run: backend("zoom_out") },
      ],
    },
    {
      title: "Chat",
      items: [
        { label: "Reload", shortcut: "F5", run: backend("reload") },
        "separator",
        { label: "New chat", shortcut: "Ctrl+N", run: backend("new_chat") },
        { label: "By number phone", shortcut: "Ctrl+M", run: act("open_chat_by_number") },
      ],
    },
    {
      title: "Users",
      items: [
        ...accounts
          .filter((a) => a.enabled)
          .map((a, i) => ({
            label: a.name || t("Account {number}", { number: i + 1 }),
            translate: false,
            shortcut: i < 9 ? `Ctrl+${i + 1}` : undefined,
            run: () => accountsApi.activate(a.id),
          })),
        "separator" as const,
        { label: "New account", shortcut: "Ctrl+U", run: act("add_account") },
      ],
    },
    {
      title: "Help",
      items: [
        { label: "Keyboard shortcuts", shortcut: "Ctrl+?", run: act("show_shortcuts") },
        { label: "Report a problem", run: act("report_problem") },
        { label: "About", run: act("about") },
      ],
    },
  ]);

  // Native popup menus: HTML dropdowns would sit under the account webviews.
  async function popup(menu: { title: string; items: Entry[] }, event: MouseEvent) {
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    const items = await Promise.all(
      menu.items.map((item) =>
        item === "separator"
          ? PredefinedMenuItem.new({ item: "Separator" })
          : item.checked !== undefined
            ? CheckMenuItem.new({
                text: item.translate === false ? item.label : t(item.label),
                checked: item.checked,
                accelerator: item.shortcut,
                action: item.run,
              })
            : MenuItem.new({
                text: item.translate === false ? item.label : t(item.label),
                accelerator: item.shortcut,
                action: item.run,
              }),
      ),
    );
    const native = await Menu.new({ items });
    await native.popup(new LogicalPosition(rect.left * scale, rect.bottom * scale));
  }
</script>

<nav class="menubar" aria-label={t("Menu bar")}>
  {#each menus as menu (menu.title)}
    <button class="title" aria-haspopup="menu" onclick={(e) => popup(menu, e)}>{t(menu.title)}</button>
  {/each}
</nav>

<style>
  .menubar {
    display: flex;
    gap: 2px;
    padding: 2px 6px;
    background: var(--sidebar-bg);
    border-bottom: 1px solid var(--csr-mid);
    font-size: 13px;
  }
  .title {
    padding: 4px 10px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .title:hover {
    background: var(--csr-hover);
  }
</style>
