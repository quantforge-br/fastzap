// Account context menu of the sidebar (ZapZap's browser account menu), as a
// native popup so it is not hidden under the account webviews.
import { CheckMenuItem, Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";

import { t } from "$lib/i18n/index.svelte";
import { accounts as accountsApi, type Account } from "$lib/ipc";

import { DEFAULT_ACCOUNT, setDoNotDisturb, setEnabled } from "./actions";

export async function showAccountMenu(
  account: Account,
  handlers: { onedit: () => void; onremove: () => void },
) {
  const items = [
    await MenuItem.new({
      text: t("Sync profile photo"),
      enabled: account.enabled,
      action: () => accountsApi.syncPhoto(account.id),
    }),
    await MenuItem.new({ text: t("Edit account"), action: handlers.onedit }),
    await PredefinedMenuItem.new({ item: "Separator" }),
    await CheckMenuItem.new({
      text: t("Do not disturb"),
      checked: !account.notifications,
      enabled: account.enabled,
      action: () => setDoNotDisturb(account.id, account.notifications),
    }),
    await CheckMenuItem.new({
      text: t("Disable account"),
      checked: !account.enabled,
      action: () => setEnabled(account.id, !account.enabled),
    }),
    await PredefinedMenuItem.new({ item: "Separator" }),
    await MenuItem.new({
      text: t("Remove account"),
      enabled: account.id !== DEFAULT_ACCOUNT,
      action: handlers.onremove,
    }),
  ];
  await (await Menu.new({ items })).popup();
}
