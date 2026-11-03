// Typed wrappers around the FastZap IPC surface (see src-tauri/src/commands).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Account } from "./generated/Account";
import type { AppInfo } from "./generated/AppInfo";
import type { PerformanceSettings } from "./generated/PerformanceSettings";
import type { ProxySettings } from "./generated/ProxySettings";
import type { AppearanceSettings } from "./generated/AppearanceSettings";
import type { AccountPatch } from "./generated/AccountPatch";
import type { ChatTarget } from "./generated/ChatTarget";
import type { LayoutState } from "./generated/LayoutState";
import type { NewAccount } from "./generated/NewAccount";
import type { DownloadDecision } from "./generated/DownloadDecision";
import type { DownloadItem } from "./generated/DownloadItem";
import type { DownloadPrompt } from "./generated/DownloadPrompt";
import type { PermissionPrompt } from "./generated/PermissionPrompt";
import type { Rect } from "./generated/Rect";
import type { RegionCallingCode } from "./generated/RegionCallingCode";
import type { ProblemForm } from "./generated/ProblemForm";
import type { ReportHandoff } from "./generated/ReportHandoff";
import type { ReportStatus } from "./generated/ReportStatus";
import type { ReportView } from "./generated/ReportView";
import type { Settings } from "./generated/Settings";
import type { SpellState } from "./generated/SpellState";
import type { UiAction } from "./generated/UiAction";
import type { UnreadSnapshot } from "./generated/UnreadSnapshot";

export type {
  Account,
  AppInfo,
  PerformanceSettings,
  ProxySettings,
  AppearanceSettings,
  AccountPatch,
  ChatTarget,
  LayoutState,
  NewAccount,
  DownloadDecision,
  DownloadItem,
  DownloadPrompt,
  PermissionPrompt,
  Rect,
  RegionCallingCode,
  ProblemForm,
  ReportHandoff,
  ReportStatus,
  ReportView,
  Settings,
  SpellState,
  UiAction,
  UnreadSnapshot,
};

export const events = {
  settingsChanged: "settings://changed",
  accountsChanged: "accounts://changed",
  layoutChanged: "layout://changed",
  unreadChanged: "unread://changed",
  uiAction: "ui://action",
  downloadsChanged: "downloads://changed",
  downloadPrompt: "downloads://prompt",
  downloadSaved: "downloads://saved",
  permissionRequest: "permissions://request",
  spellcheckChanged: "spellcheck://changed",
} as const;

export const settings = {
  get: () => invoke<Settings>("settings_get"),
  patch: (patch: unknown) => invoke<Settings>("settings_patch", { patch }),
};

export const accounts = {
  list: () => invoke<Account[]>("accounts_list"),
  create: (account: NewAccount) => invoke<Account>("accounts_create", { account }),
  update: (id: number, patch: AccountPatch) => invoke<Account>("accounts_update", { id, patch }),
  remove: (id: number) => invoke<void>("accounts_delete", { id }),
  reorder: (ids: number[]) => invoke<void>("accounts_reorder", { ids }),
  userAgentPresets: () => invoke<string[]>("user_agent_presets"),
  activate: (id: number) => invoke<void>("account_activate", { id }),
  syncPhoto: (id: number) => invoke<void>("account_sync_photo", { id }),
  pickPhoto: (title: string) => invoke<string | null>("account_photo_pick", { title }),
};

export const downloads = {
  list: () => invoke<DownloadItem[]>("downloads_list"),
  answer: (id: number, decision: DownloadDecision) => invoke<boolean>("download_answer", { id, decision }),
  clearHistory: () => invoke<void>("downloads_clear_history"),
  remove: (path: string) => invoke<void>("download_remove", { path }),
  deleteFile: (path: string) => invoke<void>("download_delete_file", { path }),
  open: (path: string) => invoke<void>("download_open", { path }),
  reveal: (path: string) => invoke<void>("download_reveal", { path }),
};

export const spellcheck = {
  state: () => invoke<SpellState>("spell_state"),
  setLanguages: (languages: string[]) => invoke<void>("spell_set_languages", { languages }),
  install: (tag: string) => invoke<void>("spell_install", { tag }),
  remove: (tag: string, disable: boolean) => invoke<void>("spell_remove", { tag, disable }),
  import: () => invoke<string | null>("spell_import"),
  openFolder: () => invoke<void>("spell_open_folder"),
};

export const reporting = {
  preview: (form: ProblemForm) => invoke<ReportView>("report_preview", { form }),
  save: (id: string | null, document: Record<string, unknown>) =>
    invoke<ReportHandoff>("report_save", { id, document }),
  openGithub: (id: string) => invoke<boolean>("report_open_github", { id }),
  list: () => invoke<ReportView[]>("report_list"),
  pending: () => invoke<ReportView | null>("report_pending"),
  keep: (id: string) => invoke<void>("report_keep", { id }),
  remove: (id: string) => invoke<void>("report_delete", { id }),
  clear: () => invoke<void>("report_clear"),
  openFolder: () => invoke<void>("report_open_folder"),
};

export const layout = {
  get: () => invoke<LayoutState>("layout_get"),
  setBounds: (bounds: Rect) => invoke<void>("layout_set_bounds", { bounds }),
  setOverlay: (overlay: boolean) => invoke<void>("layout_set_overlay", { overlay }),
};

export const app = {
  action: (action: string) => invoke<void>("app_action", { action }),
  closeConfirmed: (quit: boolean, dontAskAgain: boolean) =>
    invoke<void>("app_close_confirmed", { quit, dontAskAgain }),
  unread: () => invoke<UnreadSnapshot>("unread_get"),
  desktop: () => invoke<string>("app_desktop"),
  locale: () => invoke<string>("app_locale"),
  info: () => invoke<AppInfo>("app_info"),
  panelSupportsPositioning: () => invoke<boolean>("panel_supports_positioning"),
  setProxyPassword: (password: string) => invoke<void>("proxy_password_set", { password }),
  proxyPasswordPresent: () => invoke<boolean>("proxy_password_present"),
  resetSettings: () => invoke<void>("settings_reset"),
  pickFolder: (start?: string) => invoke<string | null>("dialog_pick_folder", { start: start ?? null }),
  pickFiles: (title: string, filterName: string, extensions: string[], multiple = false) =>
    invoke<string[]>("dialog_pick_files", { title, filterName, extensions, multiple }),
  openPath: (path: string) => invoke<void>("open_path", { path }),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  downloadsDefaultDir: () => invoke<string>("downloads_default_dir"),
  callingCodes: () => invoke<RegionCallingCode[]>("calling_codes"),
  openChatByNumber: (target: ChatTarget, accountId?: number) =>
    invoke<void>("open_chat_by_number", { target, accountId: accountId ?? null }),
};

export function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}
