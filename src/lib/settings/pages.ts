// Settings pages in ZapZap's order. Ids are stable; labels are translated.
import type { Component } from "svelte";

import Accounts from "./pages/Accounts.svelte";
import Appearance from "./pages/Appearance.svelte";
import Notifications from "./pages/Notifications.svelte";
import Permissions from "./pages/Permissions.svelte";
import SystemStartup from "./pages/SystemStartup.svelte";
import LanguageDownloads from "./pages/LanguageDownloads.svelte";
import NetworkPrivacy from "./pages/NetworkPrivacy.svelte";
import Performance from "./pages/Performance.svelte";
import About from "./pages/About.svelte";

export interface SettingsPage {
  id: string;
  label: string;
  component: Component;
}

export const PAGES: SettingsPage[] = [
  { id: "accounts", label: "Accounts", component: Accounts },
  { id: "appearance", label: "Appearance", component: Appearance },
  { id: "notifications", label: "Notifications", component: Notifications },
  { id: "permissions", label: "Permissions", component: Permissions },
  { id: "system_startup", label: "System and startup", component: SystemStartup },
  { id: "language_downloads", label: "Language and Download", component: LanguageDownloads },
  { id: "network_privacy", label: "Privacy and Network", component: NetworkPrivacy },
  { id: "performance_experimental", label: "Performance experimental", component: Performance },
  { id: "about", label: "About", component: About },
];

export const DEFAULT_PAGE = PAGES[0].id;
