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
import type { UpdateInfo } from "./generated/UpdateInfo";
import type { UnreadSnapshot } from "./generated/UnreadSnapshot";
import type { ChatMessage } from "./generated/ChatMessage";
import type { ChatSummary } from "./generated/ChatSummary";
import type { Connection } from "./generated/Connection";
import type { EvolutionInfo } from "./generated/EvolutionInfo";
import type { EvolutionSetup } from "./generated/EvolutionSetup";
import type { InstanceSummary } from "./generated/InstanceSummary";
import type { QrCode } from "./generated/QrCode";
import type { EvolutionEvent } from "./generated/EvolutionEvent";

export type {
  ChatMessage,
  ChatSummary,
  Connection,
  EvolutionEvent,
  EvolutionInfo,
  EvolutionSetup,
  InstanceSummary,
  QrCode,
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
  UpdateInfo,
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
  updateAvailable: "update://available",
  evolution: "evolution://event",
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
  copied: (id: string) => invoke<void>("report_copied", { id }),
  openGithub: (id: string) => invoke<boolean>("report_open_github", { id }),
  list: () => invoke<ReportView[]>("report_list"),
  pending: () => invoke<ReportView | null>("report_pending"),
  keep: (id: string) => invoke<void>("report_keep", { id }),
  remove: (id: string) => invoke<void>("report_delete", { id }),
  clear: () => invoke<void>("report_clear"),
  openFolder: () => invoke<void>("report_open_folder"),
};

type Target = { accountId: number; chat: string };

export const evolution = {
  instances: (baseUrl: string, adminKey: string) =>
    invoke<InstanceSummary[]>("evo_instances", { baseUrl, adminKey }),
  setup: (setup: EvolutionSetup) => invoke<Account>("evo_setup", { setup }),
  info: (accountId: number) => invoke<EvolutionInfo | null>("evo_info", { accountId }),
  connection: (accountId: number) => invoke<Connection>("evo_connection", { accountId }),
  reconnect: (accountId: number) => invoke<void>("evo_reconnect", { accountId }),
  qr: (accountId: number) => invoke<QrCode>("evo_qr", { accountId }),
  openPasskey: (accountId: number) => invoke<void>("evo_open_passkey", { accountId }),
  pairPhone: (accountId: number, phone: string) => invoke<string>("evo_pair_phone", { accountId, phone }),
  logout: (accountId: number) => invoke<void>("evo_logout", { accountId }),
  chats: (accountId: number) => invoke<ChatSummary[]>("evo_chats", { accountId }),
  messages: (t: Target, before: number | null, limit: number) =>
    invoke<ChatMessage[]>("evo_messages", { ...t, before, limit }),
  setOpenChat: (accountId: number, chat: string | null) =>
    invoke<void>("evo_set_open_chat", { accountId, chat }),
  markRead: (t: Target) => invoke<void>("evo_mark_read", t),
  typing: (t: Target, composing: boolean) => invoke<void>("evo_typing", { ...t, composing }),
  sendText: (t: Target, text: string, quote: string | null) =>
    invoke<void>("evo_send_text", { ...t, text, quote }),
  sendFile: (t: Target, caption: string, quote: string | null) =>
    invoke<boolean>("evo_send_file", { ...t, caption, quote }),
  retry: (t: Target, id: string) => invoke<void>("evo_retry", { ...t, id }),
  react: (t: Target, id: string, emoji: string) => invoke<void>("evo_react", { ...t, id, emoji }),
  edit: (t: Target, id: string, text: string) => invoke<void>("evo_edit", { ...t, id, text }),
  remove: (t: Target, id: string) => invoke<void>("evo_delete", { ...t, id }),
  media: (t: Target, id: string) => invoke<string>("evo_media", { ...t, id }),
  saveMedia: (t: Target, id: string) => invoke<boolean>("evo_save_media", { ...t, id }),
  loadOlder: (t: Target) => invoke<boolean>("evo_load_older", t),
  avatar: (accountId: number, jid: string) => invoke<string | null>("evo_avatar", { accountId, jid }),
  refreshContacts: (accountId: number) => invoke<void>("evo_refresh_contacts", { accountId }),
};

export const layout = {
  get: () => invoke<LayoutState>("layout_get"),
  setBounds: (bounds: Rect) => invoke<void>("layout_set_bounds", { bounds }),
  setOverlay: (overlay: boolean) => invoke<void>("layout_set_overlay", { overlay }),
};

export const app = {
  updateInfo: () => invoke<UpdateInfo | null>("update_info"),
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
