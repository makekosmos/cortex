import { computed, shallowRef } from "vue";
import { gamesApi } from "../../src/lib/api";
import type { Game, NewGame } from "../../src/types";
import {
  cleanNameFromFile,
  fileNameFromPath,
  findMergeCandidate,
} from "../lib/gameImport";
import { useDropZone } from "./useDropZone";

type ToastPayload = {
  tone: "success" | "warning" | "error";
  title: string;
  description?: string;
};

export interface LibraryGameImportOptions {
  games: () => readonly Game[];
  addGames: (games: NewGame[]) => Promise<Game[]>;
  updateGame: (id: string, updates: { exe_path: string }) => Promise<unknown>;
  notify: (payload: ToastPayload) => void;
  resolveShortcutTarget?: (path: string) => Promise<string>;
  existsByPath?: (path: string) => Promise<boolean>;
  confirm?: (message: string) => boolean;
  log?: Pick<Console, "error">;
}

export function useLibraryGameImport(options: LibraryGameImportOptions) {
  const metadataQueue = shallowRef<Game[]>([]);
  const resolveShortcutTarget =
    options.resolveShortcutTarget ?? gamesApi.resolveShortcutTarget;
  const existsByPath = options.existsByPath ?? gamesApi.existsByPath;
  const confirm = options.confirm ?? window.confirm;
  const log = options.log ?? console;

  function enqueueMetadata(added: readonly Game[]) {
    metadataQueue.value = [
      ...metadataQueue.value,
      ...added.filter(
        (candidate) =>
          !metadataQueue.value.some((queued) => queued.id === candidate.id),
      ),
    ];
  }

  function advanceMetadataQueue() {
    metadataQueue.value = metadataQueue.value.slice(1);
  }

  function clearMetadataQueue() {
    metadataQueue.value = [];
  }

  async function handleDroppedPaths(paths: string[]) {
    if (paths.length === 0) return;

    const toAdd: NewGame[] = [];
    const seen = new Set<string>();
    let skipped = 0;
    let invalid = 0;
    let merged = 0;

    for (const rawPath of paths) {
      let resolved = rawPath;
      if (rawPath.toLowerCase().endsWith(".lnk")) {
        try {
          resolved = await resolveShortcutTarget(rawPath);
        } catch (cause) {
          log.error?.("Failed to resolve shortcut:", cause);
          invalid += 1;
          continue;
        }
      }

      const lowerResolved = resolved.toLowerCase();
      if (!lowerResolved.endsWith(".exe")) {
        invalid += 1;
        continue;
      }

      if (seen.has(lowerResolved)) {
        continue;
      }

      seen.add(lowerResolved);
      const exists = await existsByPath(resolved).catch(() => false);
      if (exists) {
        skipped += 1;
        continue;
      }

      const exeName = fileNameFromPath(resolved);
      const name = cleanNameFromFile(exeName);
      const candidate = findMergeCandidate([...options.games()], name);
      if (candidate) {
        const doMerge = confirm(
          `Найдена игра с похожим названием: "${candidate.name}" из библиотеки. Обновить путь для существующей записи?`,
        );

        if (doMerge) {
          await options.updateGame(candidate.id, { exe_path: resolved });
          merged += 1;
          continue;
        }
      }

      toAdd.push({
        name,
        exe_path: resolved,
        exe_name: exeName,
      });
    }

    if (toAdd.length > 0) {
      try {
        const added = await options.addGames(toAdd);
        enqueueMetadata(added);

        const extra: string[] = [];
        if (skipped > 0) extra.push(`Пропущено: ${skipped}`);
        if (invalid > 0) extra.push(`Не поддерживается: ${invalid}`);
        if (merged > 0) {
          extra.push(`Объединено с существующими: ${merged}`);
        }

        options.notify({
          tone: "success",
          title: `Добавлено ${added.length}`,
          description: extra.length ? extra.join(" | ") : undefined,
        });
      } catch (cause) {
        log.error?.("Failed to add dropped games:", cause);
        options.notify({
          tone: "error",
          title: "Не удалось добавить игру",
          description: "Проверьте путь к файлу.",
        });
      }
      return;
    }

    if (merged > 0) {
      options.notify({
        tone: "success",
        title: `Объединено с существующими: ${merged}`,
        description: "Обновлены пути для игр с совпавшим названием.",
      });
      return;
    }

    options.notify({
      tone: "warning",
      title: "Файлы не добавлены",
      description: "Поддерживаются .exe и .lnk.",
    });
  }

  const { dropActive, dropHandlers } = useDropZone({
    onDropPaths: handleDroppedPaths,
  });

  const currentMetadataGame = computed(() => metadataQueue.value[0] ?? null);

  return {
    metadataQueue,
    currentMetadataGame,
    dropActive,
    dropHandlers,
    enqueueMetadata,
    advanceMetadataQueue,
    clearMetadataQueue,
    handleDroppedPaths,
  };
}
