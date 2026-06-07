// useExportTab — state + handlers Export tab'а (Phase 7).
// Универсальный per-type export: список конвертеров приходит из
// kepler-backend через `window.kepler.export.list()`. UI ничего не знает
// о конкретных object_type'ах — просто показывает что зарегистрировано.

import { ref } from "vue";
import type { ExportConverterInfo, ExportResult } from "@shared/ipc-types";

export interface ExportHistoryEntry {
  converter_id: string;
  display_name: string;
  format: string;
  dest_dir: string;
  timestamp: number;
  ok: boolean;
  file_count: number;
  bytes: number;
}

const EXPORT_HISTORY_KEY = "kepler-export-history";
const EXPORT_HISTORY_LIMIT = 10;

export function useExportTab() {
  const exportConverters = ref<ExportConverterInfo[]>([]);
  const exportLoading = ref<boolean>(false);
  const exportError = ref<string>("");
  const exportSelectedFormat = ref<Record<string, string>>({});
  const exportBusyId = ref<string>("");
  const exportStatusByConverter = ref<Record<string, string>>({});
  const exportHistory = ref<ExportHistoryEntry[]>(loadExportHistory());

  function loadExportHistory(): ExportHistoryEntry[] {
    try {
      const raw = localStorage.getItem(EXPORT_HISTORY_KEY);
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed.slice(0, EXPORT_HISTORY_LIMIT);
    } catch {
      return [];
    }
  }

  function pushExportHistory(entry: ExportHistoryEntry) {
    const next = [entry, ...exportHistory.value].slice(0, EXPORT_HISTORY_LIMIT);
    exportHistory.value = next;
    try {
      localStorage.setItem(EXPORT_HISTORY_KEY, JSON.stringify(next));
    } catch {
      // ignore quota / unavailable
    }
  }

  async function loadExportConverters() {
    exportLoading.value = true;
    exportError.value = "";
    try {
      const list = await window.kepler.export.list();
      exportConverters.value = list;
      const sel = { ...exportSelectedFormat.value };
      for (const c of list) {
        if (!sel[c.converter_id]) {
          sel[c.converter_id] = c.default_format;
        }
      }
      exportSelectedFormat.value = sel;
    } catch (e) {
      exportError.value = (e as Error).message;
      exportConverters.value = [];
    } finally {
      exportLoading.value = false;
    }
  }

  async function onRunExport(c: ExportConverterInfo) {
    if (exportBusyId.value) return;
    const format = exportSelectedFormat.value[c.converter_id] ?? c.default_format;
    let destDir: string | null = null;
    try {
      destDir = await window.kepler.export.pickDir();
    } catch (e) {
      exportStatusByConverter.value = {
        ...exportStatusByConverter.value,
        [c.converter_id]: `Ошибка диалога: ${(e as Error).message}`,
      };
      return;
    }
    if (!destDir) return; // отменили

    exportBusyId.value = c.converter_id;
    exportStatusByConverter.value = {
      ...exportStatusByConverter.value,
      [c.converter_id]: "Экспортирую…",
    };
    try {
      const r: ExportResult = await window.kepler.export.run({
        converter_id: c.converter_id,
        format,
        dest_dir: destDir,
      });
      const ok = r.errors.length === 0;
      const sizeKb = (r.bytes / 1024).toFixed(1);
      const parts: string[] = [`Готово: ${r.files_written.length} файлов, ${sizeKb} KB`];
      if (r.errors.length > 0) {
        parts.push(`Ошибок: ${r.errors.length}`);
        const sample = r.errors.slice(0, 3).join("; ");
        parts.push(sample);
      }
      exportStatusByConverter.value = {
        ...exportStatusByConverter.value,
        [c.converter_id]: parts.join(" · "),
      };
      pushExportHistory({
        converter_id: c.converter_id,
        display_name: c.display_name,
        format,
        dest_dir: destDir,
        timestamp: Date.now(),
        ok,
        file_count: r.files_written.length,
        bytes: r.bytes,
      });
    } catch (e) {
      exportStatusByConverter.value = {
        ...exportStatusByConverter.value,
        [c.converter_id]: `Ошибка: ${(e as Error).message}`,
      };
    } finally {
      exportBusyId.value = "";
    }
  }

  function formatHistoryTime(ts: number): string {
    const d = new Date(ts);
    return `${d.toLocaleDateString("ru")} ${d.toLocaleTimeString("ru", {
      hour: "2-digit",
      minute: "2-digit",
    })}`;
  }

  return {
    exportConverters,
    exportLoading,
    exportError,
    exportSelectedFormat,
    exportBusyId,
    exportStatusByConverter,
    exportHistory,
    loadExportConverters,
    onRunExport,
    formatHistoryTime,
  };
}
