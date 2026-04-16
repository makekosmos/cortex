import type { Achievement, AchievementSeed } from "./shared";

export const DEFAULT_ACHIEVEMENTS: readonly AchievementSeed[] = [
  {
    id: "first-launch",
    title: "First Launch",
    description: "Launch any game once.",
    event_trigger: "game_launch",
    target: 1,
  },
  {
    id: "backup-guardian",
    title: "Backup Guardian",
    description: "Complete a backup or restore flow.",
    event_trigger: "backup_restore",
    target: 1,
  },
  {
    id: "download-scout",
    title: "Download Scout",
    description: "Register a completed download event.",
    event_trigger: "download_complete",
    target: 1,
  },
  {
    id: "scan-master",
    title: "Scan Master",
    description: "Finish a library scan.",
    event_trigger: "scan_complete",
    target: 1,
  },
] as const;

export interface AchievementRow {
  id: string;
  title: string;
  description: string;
  event_trigger: string;
  progress: number;
  target: number;
  unlocked: number;
  unlocked_at: string | null;
  created_at: string;
}

export function mapAchievementRow(row: AchievementRow): Achievement {
  return {
    id: row.id,
    title: row.title,
    description: row.description,
    event_trigger: row.event_trigger,
    progress: row.progress,
    target: row.target,
    unlocked: row.unlocked === 1,
    unlocked_at: row.unlocked_at,
    created_at: row.created_at,
  };
}

