import type { Component } from "vue";
import {
  PhArticle,
  PhCube,
  PhGameController,
  PhListChecks,
  PhNotebook,
  PhTag,
  PhTimer,
} from "@phosphor-icons/vue";

export const HIDDEN_DASHBOARD_TYPE_IDS = new Set(["blocklist_obj"]);

export interface DashboardTypeVisual {
  icon: Component;
  from: string;
  to: string;
}

const TYPE_VISUALS: Record<string, DashboardTypeVisual> = {
  note_obj: {
    icon: PhArticle,
    from: "color-mix(in srgb, var(--status-warning) 64%, var(--destructive))",
    to: "color-mix(in srgb, var(--status-warning) 40%, var(--background))",
  },
  "system-type-journal": {
    icon: PhNotebook,
    from: "color-mix(in srgb, var(--status-warning) 58%, var(--destructive))",
    to: "color-mix(in srgb, var(--status-warning) 36%, var(--background))",
  },
  time_entry_obj: {
    icon: PhTimer,
    from: "color-mix(in srgb, var(--destructive) 48%, var(--accent))",
    to: "color-mix(in srgb, var(--destructive) 54%, var(--background))",
  },
  game_obj: {
    icon: PhGameController,
    from: "color-mix(in srgb, var(--destructive) 76%, var(--accent))",
    to: "color-mix(in srgb, var(--destructive) 58%, var(--background))",
  },
  tag_obj: {
    icon: PhTag,
    from: "color-mix(in srgb, var(--status-success) 58%, var(--accent))",
    to: "color-mix(in srgb, var(--status-success) 42%, var(--background))",
  },
  task_obj: {
    icon: PhListChecks,
    from: "color-mix(in srgb, var(--accent) 82%, var(--status-success))",
    to: "color-mix(in srgb, var(--accent) 52%, var(--background))",
  },
};

const FALLBACK_VISUALS: DashboardTypeVisual[] = [
  {
    icon: PhCube,
    from: "var(--destructive)",
    to: "color-mix(in srgb, var(--destructive) 60%, var(--background))",
  },
  {
    icon: PhCube,
    from: "var(--status-warning)",
    to: "color-mix(in srgb, var(--status-warning) 60%, var(--background))",
  },
  {
    icon: PhCube,
    from: "var(--status-success)",
    to: "color-mix(in srgb, var(--status-success) 60%, var(--background))",
  },
  {
    icon: PhCube,
    from: "var(--accent)",
    to: "color-mix(in srgb, var(--accent) 60%, var(--background))",
  },
  {
    icon: PhCube,
    from: "var(--accent)",
    to: "color-mix(in srgb, var(--accent) 70%, var(--background))",
  },
];

function fallbackVisualForType(id: string): DashboardTypeVisual {
  let hash = 0;
  for (let i = 0; i < id.length; i++) hash = (hash * 31 + id.charCodeAt(i)) >>> 0;
  return FALLBACK_VISUALS[hash % FALLBACK_VISUALS.length];
}

export function typeVisualFor(id: string): DashboardTypeVisual {
  return TYPE_VISUALS[id] ?? fallbackVisualForType(id);
}
