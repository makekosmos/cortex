import type { Game } from "../../src/types";

export const GAME_PATH_TOKEN = "{PATHTOGAME}";

export const PLAY_STATUS_LABELS: Record<Game["play_status"], string> = {
  not_started: "Не начато",
  in_progress: "В процессе",
  completed: "Пройдено",
  abandoned: "Брошено",
};

export const PLAY_STATUS_TONES: Record<Game["play_status"], string> = {
  not_started: "border-white/10 text-muted-foreground",
  in_progress: "border-sky-400/35 text-sky-300",
  completed: "border-emerald-400/35 text-emerald-300",
  abandoned: "border-rose-400/35 text-rose-300",
};

export function formatPlaytime(seconds: number) {
  if (!seconds) return "0 ч";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) return `${hours} ч ${minutes} мин`;
  return `${minutes} мин`;
}

export function formatPlayedHours(seconds: number) {
  if (seconds <= 0) return "0 ч";
  if (seconds < 3600) return "<1 ч";
  return `${Math.floor(seconds / 3600)} ч`;
}

export function formatBytes(bytes: number) {
  if (!bytes || bytes <= 0) return "0 Б";
  const units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  let size = bytes;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  const digits = size >= 10 ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unitIndex]}`;
}

export function normalizeDescription(value: string | null) {
  if (!value) return null;
  const plain = value.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
  return plain || null;
}

export function resolveSavePathTemplate(path: string, game?: Game | null) {
  if (!path.includes(GAME_PATH_TOKEN)) return path;
  const exePath = game?.exe_path;
  if (!exePath) return path;
  const lastSlash = Math.max(exePath.lastIndexOf("\\"), exePath.lastIndexOf("/"));
  const base = lastSlash > 0 ? exePath.slice(0, lastSlash) : exePath;
  return path.split(GAME_PATH_TOKEN).join(base);
}
