// Customization commands (src-tauri/src/commands/customizations.rs).
import { invoke } from "@tauri-apps/api/core";

import type { AssetKind } from "$lib/ipc/generated/AssetKind";
import type { CustomFile } from "$lib/ipc/generated/CustomFile";
import type { UrlImport } from "$lib/ipc/generated/UrlImport";

export type { AssetKind, CustomFile };

/** `null` is the global scope, otherwise an account id. */
export type Scope = number | null;

export const custom = {
  list: (scope: Scope, kind: AssetKind) => invoke<CustomFile[]>("custom_list", { scope, kind }),
  read: (scope: Scope, kind: AssetKind, name: string) => invoke<string>("custom_read", { scope, kind, name }),
  write: (scope: Scope, kind: AssetKind, oldName: string | null, name: string, content: string) =>
    invoke<string>("custom_write", { scope, kind, oldName, name, content }),
  importFile: (scope: Scope, kind: AssetKind) => invoke<string | null>("custom_import_file", { scope, kind }),
  importUrl: (scope: Scope, kind: AssetKind, url: string, name: string | null) =>
    invoke<UrlImport>("custom_import_url", { scope, kind, url, name }),
  duplicate: (scope: Scope, kind: AssetKind, name: string) =>
    invoke<string>("custom_duplicate", { scope, kind, name }),
  remove: (scope: Scope, kind: AssetKind, name: string) => invoke<void>("custom_delete", { scope, kind, name }),
  setFileEnabled: (scope: Scope, kind: AssetKind, name: string, enabled: boolean) =>
    invoke<void>("custom_set_file_enabled", { scope, kind, name, enabled }),
  openFolder: (scope: Scope, kind: AssetKind) => invoke<void>("custom_open_folder", { scope, kind }),
  pickPreview: (scope: Scope, name: string) => invoke<string | null>("custom_pick_preview", { scope, name }),
  preview: (scope: Scope, name: string) => invoke<string | null>("custom_preview", { scope, name }),
  reload: (scope: Scope) => invoke<void>("custom_reload", { scope }),
};

/** Settings key of an account scope (ZapZap: the default account is `storage-whats`). */
export function accountKey(id: number): string {
  return id === 1 ? "storage-whats" : String(id);
}
