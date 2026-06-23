import { existsSync, lstatSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { resolveInstance } from "./instance";
import { keplerDataDir } from "./data-dir";
import type { StorageSummary, StorageSummaryItem } from "../shared/ipc-types";

function safeSizeOfPath(target: string): number {
  try {
    if (!existsSync(target)) return 0;
    const stat = lstatSync(target);
    if (stat.isSymbolicLink()) return 0;
    if (stat.isFile()) return stat.size;
    if (!stat.isDirectory()) return 0;
    let total = 0;
    for (const entry of readdirSync(target, { withFileTypes: true })) {
      const child = path.join(target, entry.name);
      try {
        if (entry.isSymbolicLink()) continue;
        if (entry.isDirectory()) {
          total += safeSizeOfPath(child);
        } else if (entry.isFile()) {
          total += statSync(child).size;
        }
      } catch {
        // Best-effort summary: skip files that are locked or disappear mid-scan.
      }
    }
    return total;
  } catch {
    return 0;
  }
}

function safeSizeOfFiles(dir: string, names: string[]): number {
  return names.reduce((sum, name) => sum + safeSizeOfPath(path.join(dir, name)), 0);
}

export function isPathInside(parent: string, child: string): boolean {
  const rel = path.relative(parent, child);
  return rel === "" || (Boolean(rel) && !rel.startsWith("..") && !path.isAbsolute(rel));
}

function dictationSharedAssetsRoot(): string {
  const override = process.env.KOSMOS_LOCAL_STT_DIR?.trim();
  if (override) return override;
  return path.join(
    process.env.APPDATA || process.env.XDG_CONFIG_HOME || process.env.HOME || ".",
    "Kosmos",
  );
}

function storageSummaryItem(
  id: string,
  label: string,
  target: string,
  description?: string,
): StorageSummaryItem {
  return {
    id,
    label,
    path: target,
    bytes: safeSizeOfPath(target),
    exists: existsSync(target),
    description,
  };
}

export function buildStorageSummary(): StorageSummary {
  const dataDir = keplerDataDir();
  const userDataDir = resolveInstance().userDataDir;
  const dictationAssetsRoot = dictationSharedAssetsRoot();
  const dictationModelsPath = path.join(dictationAssetsRoot, "models", "dictation");
  const dictationToolsPath = path.join(dictationAssetsRoot, "tools", "dictation");
  const dataDirBytes = safeSizeOfPath(dataDir);
  const userDataBytes = safeSizeOfPath(userDataDir);
  const arkBytes = safeSizeOfFiles(dataDir, ["ark.db", "ark.db-wal", "ark.db-shm"]);
  const indexBytes = safeSizeOfFiles(dataDir, [
    "app-index.db",
    "app-index.db-wal",
    "app-index.db-shm",
    "file-index.db",
    "file-index.db-wal",
    "file-index.db-shm",
  ]);

  const items: StorageSummaryItem[] = [
    {
      id: "ark-db",
      label: "База ARK",
      path: path.join(dataDir, "ark.db"),
      bytes: arkBytes,
      exists: existsSync(path.join(dataDir, "ark.db")),
      description: "Основная база объектов и синхронизации.",
    },
    {
      id: "indexes",
      label: "Индексы поиска",
      path: dataDir,
      bytes: indexBytes,
      exists:
        existsSync(path.join(dataDir, "app-index.db")) ||
        existsSync(path.join(dataDir, "file-index.db")),
      description: "Индексы приложений и файлов, их можно пересоздать.",
    },
    storageSummaryItem(
      "dictation-models",
      "Локальные модели диктации",
      dictationModelsPath,
      "Скачанные Whisper-модели.",
    ),
    storageSummaryItem(
      "dictation-tools",
      "Локальные инструменты диктации",
      dictationToolsPath,
      "whisper.cpp и вспомогательные файлы.",
    ),
    storageSummaryItem("extensions", "Установленные расширения", path.join(dataDir, "extensions")),
    storageSummaryItem(
      "extensions-data",
      "Данные расширений",
      path.join(dataDir, "extensions-data"),
    ),
    storageSummaryItem("logs", "Логи", path.join(dataDir, "logs")),
    storageSummaryItem("crashes", "Крэши", path.join(dataDir, "crashes")),
  ];

  const knownDataBytes = items
    .filter((item) => isPathInside(dataDir, item.path))
    .reduce((sum, item) => sum + item.bytes, 0);
  items.push({
    id: "other-data",
    label: "Прочие данные Kosmos",
    path: dataDir,
    bytes: Math.max(0, dataDirBytes - knownDataBytes),
    exists: existsSync(dataDir),
  });
  items.push({
    id: "electron-user-data",
    label: "Состояние окна и Chromium",
    path: userDataDir,
    bytes: userDataBytes,
    exists: existsSync(userDataDir),
    description: "Electron userData: кэши, Local Storage, состояние UI.",
  });

  const totalBytes =
    dataDirBytes +
    (isPathInside(dataDir, userDataDir) ? 0 : userDataBytes) +
    [dictationModelsPath, dictationToolsPath]
      .filter((p) => !isPathInside(dataDir, p) && !isPathInside(userDataDir, p))
      .reduce((sum, p) => sum + safeSizeOfPath(p), 0);

  return {
    dataDir,
    userDataDir,
    totalBytes,
    items,
  };
}
