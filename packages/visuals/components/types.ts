// Public types для компонентов @kosmos/visuals. Вынесены из `<script setup>`
// файлов потому что `tsc` (без vue-tsc) не видит named exports из script
// setup — только default. Re-export через `index.ts` теперь работает.

import type { Component } from "vue";

// --- Sidebar ----------------------------------------------------------------

export interface SidebarConfig {
  width: number;
  hidden: boolean;
}

export interface SidebarNavItem {
  id: string;
  icon: Component;
  to?: string;
  label?: string;
  active?: boolean;
  testId?: string;
  onClick?: () => void;
}

export interface SidebarProjectItem {
  id: string;
  label: string;
  to?: string;
  active?: boolean;
  colorClass?: string;
  color?: string;
  iconSrc?: string;
  iconColor?: string;
  testId?: string;
  onClick?: () => void;
  /** ПКМ-меню. Receiver сам делает `event.preventDefault()` и
      открывает `ContextMenu` (см. `useContextMenu` в @kosmos/visuals). */
  onContextMenu?: (event: MouseEvent) => void;
}

export interface SidebarProjectGroup {
  id: string;
  label: string;
  items: SidebarProjectItem[];
  defaultCollapsed?: boolean;
  actionIcon?: Component;
  actionLabel?: string;
  actionTestId?: string;
  onAction?: () => void;
}

// --- Titlebar ---------------------------------------------------------------

export type TitlebarPlatform = "mac" | "windows" | "linux";

// --- StatusDot --------------------------------------------------------------

export type StatusDotTone = "success" | "warning" | "danger" | "neutral";

// --- TodoRow ----------------------------------------------------------------

export interface TodoRowItem {
  id: string;
  title: string;
  notes?: string | null;
  isCompleted?: boolean;
  isCancelled?: boolean;
  isTrashed?: boolean;
  scheduledDate?: string | null;
  billable?: boolean;
  price?: number | null;
}

export interface TodoDropPayload {
  targetId: string;
  after: boolean;
}

export interface TodoRowUpdate {
  title?: string;
  notes?: string | null;
  scheduledDate?: string | null;
  billable?: boolean;
  price?: number | null;
}

// --- QuickEntryPanel --------------------------------------------------------

export interface QuickEntryProject {
  id: string;
  title: string;
  billable?: boolean;
}

export interface QuickEntrySavePayload {
  title: string;
  notes: string | null;
  scheduledDate: string | null;
  projectId: string | null;
  billable: boolean;
  price: number | null;
}
