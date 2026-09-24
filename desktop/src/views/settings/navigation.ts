// Public settings navigation contract.
//
// Static data lives in `navigation.data.ts`; this module keeps the shared
// types and stable imports used by `SettingsView.vue` and other callers.

import type { Component } from "vue";

export type Tab = "general" | "about" | "debug" | "file-index" | "export";

export interface SettingsNavigationItem {
  tab: Tab;
  label: string;
  group: "main" | "advanced";
  layout: "basic" | "advanced";
  icon: Component;
  iconGradient?: { from: string; to: string };
  sidebarImage?: string;
  introImage?: string;
  description?: string;
  keywords: string[];
}

export type AppSettingsTab = string;

export interface AppCommandSetting {
  id: string;
  title: string;
  icon: string;
  iconFrom: string;
  iconTo: string;
  shortcut?: string;
}

export const HIDDEN_COMMANDS_KEY = "kepler.launcher.hiddenCommandIds";

export { settingsNavigationItems } from "./navigation.data";
