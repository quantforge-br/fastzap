// Account actions shared by the Accounts page and the "add_account" shortcut.
import { accounts as accountsApi, type Account } from "$lib/ipc";

import { newIconSvg } from "./icon";

/** ZapZap creates accounts right away, unnamed and with a generated icon. */
export function addAccount(): Promise<Account> {
  return accountsApi.create({ name: "", icon: newIconSvg(), kind: "whatsapp_web", evolution: null });
}

/** Account 1 is ZapZap's `storage-whats`, which cannot be removed. */
export const DEFAULT_ACCOUNT = 1;

const EMPTY_PATCH = {
  name: null,
  icon: undefined,
  enabled: null,
  zoom: null,
  user_agent: null,
  notifications: null,
  evolution: null,
};

export function setEnabled(id: number, enabled: boolean) {
  return accountsApi.update(id, { ...EMPTY_PATCH, enabled });
}

/** `notifications: false` is "do not disturb". */
export function setDoNotDisturb(id: number, dnd: boolean) {
  return accountsApi.update(id, { ...EMPTY_PATCH, notifications: !dnd });
}
