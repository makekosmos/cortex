import {
  ExternalLink,
  File as FileIcon,
  FolderOpen,
  Loader2,
  Pencil,
  RefreshCw,
  Save,
  Search,
  Sparkles,
  X,
} from "lucide-react";
import type { ReactNode } from "react";
import { useMemo, useState } from "react";
import { useToast } from "@/components/ToastProvider";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Switch } from "@/components/ui/switch";
import { useSettingsState } from "@/hooks/useSettingsState";
import { backupApi } from "@/lib/api";
import { openPath, pickDirectoryPath, pickFilePath } from "@/lib/browser";
import { cn } from "@/lib/utils";
import { useGames, useGamesState } from "@/store/GamesContext";
import type { BackupInfo, Game, SavePathLookup } from "@/types";

const GAME_PATH_TOKEN = "{PATHTOGAME}";

type LookupState<T> = {
  loading: boolean;
  data?: T;
  error?: string;
};

const formatBytes = (bytes: number) => {
  if (!bytes || bytes <= 0) return "0 Б";
  const units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  let size = bytes;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  const digits = size >= 10 ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unit]}`;
};

const resolveSavePathTemplate = (path: string, game?: Game | null) => {
  if (!path.includes(GAME_PATH_TOKEN)) return path;
  const exePath = game?.exe_path;
  if (!exePath) return path;
  const lastSlash = Math.max(
    exePath.lastIndexOf("\\"),
    exePath.lastIndexOf("/"),
  );
  const base = lastSlash > 0 ? exePath.slice(0, lastSlash) : exePath;
  return path.replace(/{PATHTOGAME}/g, base);
};

function PathPreview({ value }: { value: string }) {
  if (!value.includes(GAME_PATH_TOKEN)) return null;
  const parts = value.split(GAME_PATH_TOKEN);
  return (
    <div className="text-[11px] text-muted-foreground leading-relaxed">
      Путь:{" "}
      {parts.map((part, index) => (
        <span key={`${index}-${part}`}>
          {part}
          {index < parts.length - 1 && (
            <span className="mx-1 rounded-md border border-emerald-500/35 bg-emerald-500/10 px-1 py-0.5 font-mono text-[10px] text-emerald-300">
              {GAME_PATH_TOKEN}
            </span>
          )}
        </span>
      ))}
    </div>
  );
}

function InfoCard({
  title,
  description,
  icon,
  className,
}: {
  title: string;
  description: string;
  icon: ReactNode;
  className: string;
}) {
  return (
    <div
      className={cn(
        // Tall, rectangular promo card.
        "relative overflow-hidden rounded-xl border p-5 min-h-[240px] sm:min-h-[260px] lg:min-h-[320px] flex flex-col",
        "shadow-[0_18px_55px_rgba(8,12,24,0.30)]",
        "hover:-translate-y-0.5 hover:shadow-[0_24px_80px_rgba(8,12,24,0.38)] transition-all duration-200",
        "backdrop-blur-xl",
        className,
      )}
    >
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_15%_0%,rgba(255,255,255,0.22),transparent_55%),radial-gradient(circle_at_80%_85%,rgba(255,255,255,0.12),transparent_60%)] opacity-70" />
      <div className="pointer-events-none absolute -top-14 -right-14 h-40 w-40 rounded-full bg-white/10 blur-2xl" />
      <div className="pointer-events-none absolute -bottom-20 -left-20 h-56 w-56 rounded-full bg-white/5 blur-3xl" />

      <div className="relative flex-1 flex flex-col">
        <div className="flex items-center justify-between gap-3">
          <div className="flex h-12 w-12 items-center justify-center rounded-xl border border-white/12 bg-white/5 shadow-[0_12px_30px_rgba(0,0,0,0.22)]">
            {icon}
          </div>
          <div className="hidden sm:flex items-center gap-2">
            <span className="rounded-full border border-white/10 bg-white/5 px-2.5 py-1 text-[10px] text-muted-foreground">
              SQOBA
            </span>
          </div>
        </div>

        <div className="mt-4 text-base font-semibold tracking-tight">
          {title}
        </div>
        <div className="mt-2 text-xs text-muted-foreground leading-relaxed">
          {description}
        </div>

        <div className="mt-auto pt-4">
          <div className="h-px w-full bg-gradient-to-r from-white/20 via-white/0 to-transparent" />
          <div className="mt-3 text-[11px] text-muted-foreground">
            Нажмите на SQOBA, чтобы настроить.
          </div>
        </div>
      </div>
    </div>
  );
}

export default function SqobaPage() {
  const { notify } = useToast();
  const { games, loading: gamesLoading } = useGamesState();
  const { updateGame, refreshGames } = useGames();

  const {
    loading: settingsLoading,
    saving: settingsSaving,
    backupDirectory,
    setBackupDirectory,
    autoBackup,
    setAutoBackup,
    backupBeforeLaunch,
    setBackupBeforeLaunch,
    compressionEnabled,
    handleCompressionToggle,
    compressionLevel,
    handleCompressionLevelChange,
    maxBackups,
    handleMaxBackupsChange,
    saveSettings,
    selectBackupDirectory,
    refreshSqobaManifest,
  } = useSettingsState();

  const [aboutOpen, setAboutOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [onlyMissing, setOnlyMissing] = useState(false);
  const [scanAll, setScanAll] = useState<LookupState<null>>({ loading: false });

  const [pathsByGameId, setPathsByGameId] = useState<
    Record<string, LookupState<SavePathLookup>>
  >({});
  const [filesByGameId, setFilesByGameId] = useState<
    Record<string, LookupState<BackupInfo | null>>
  >({});

  const [editingGameId, setEditingGameId] = useState<string | null>(null);
  const [savePathDraft, setSavePathDraft] = useState("");
  const [savingSavePath, setSavingSavePath] = useState(false);

  const filteredGames = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return games.filter((game) => {
      if (onlyMissing && game.save_path) return false;
      if (!needle) return true;
      return game.name.toLowerCase().includes(needle);
    });
  }, [games, onlyMissing, query]);

  const missingCount = useMemo(
    () => games.filter((g) => !g.save_path).length,
    [games],
  );

  const openSavePath = async (game: Game, path: string) => {
    try {
      await openPath(resolveSavePathTemplate(path, game));
    } catch (e) {
      console.error("Failed to open save path:", e);
      notify({
        tone: "error",
        title: "Не удалось открыть путь",
        description: "Проверьте, что путь существует и доступен.",
      });
    }
  };

  const loadSavePaths = async (game: Game) => {
    setPathsByGameId((prev) => ({
      ...prev,
      [game.id]: { loading: true },
    }));

    try {
      const result = await backupApi.findGameSavePaths(game.name, game.id);
      setPathsByGameId((prev) => ({
        ...prev,
        [game.id]: { loading: false, data: result },
      }));

      if (!result.save_path) {
        notify({
          tone: "warning",
          title: "Сохранения не найдены",
          description: `Для "${game.name}" не удалось определить путь автоматически.`,
        });
        return;
      }

      if (result.candidates.length > 1) {
        notify({
          tone: "info",
          title: "Найдено несколько вариантов",
          description:
            "SQOBA нашёл несколько путей. Откройте и проверьте нужный, либо задайте путь вручную.",
        });
      }
    } catch (e) {
      console.error("Failed to locate saves:", e);
      setPathsByGameId((prev) => ({
        ...prev,
        [game.id]: {
          loading: false,
          error: e instanceof Error ? e.message : "Ошибка поиска",
        },
      }));
      notify({
        tone: "error",
        title: "Ошибка поиска сохранений",
        description: `Не удалось найти сохранения для "${game.name}".`,
      });
    }
  };

  const loadSaveFiles = async (game: Game) => {
    setFilesByGameId((prev) => ({
      ...prev,
      [game.id]: { loading: true },
    }));

    try {
      const result = await backupApi.findGameSaves(game.name, game.id);
      setFilesByGameId((prev) => ({
        ...prev,
        [game.id]: { loading: false, data: result },
      }));

      if (!result?.save_path) {
        notify({
          tone: "warning",
          title: "Файлы сохранений не найдены",
          description:
            "Если у игры нет стандартного пути, укажите его вручную (папка или файл).",
        });
      }
    } catch (e) {
      console.error("Failed to load save files:", e);
      setFilesByGameId((prev) => ({
        ...prev,
        [game.id]: {
          loading: false,
          error: e instanceof Error ? e.message : "Ошибка загрузки",
        },
      }));
      notify({
        tone: "error",
        title: "Ошибка загрузки файлов",
        description: `Не удалось получить список файлов для "${game.name}".`,
      });
    }
  };

  const beginEdit = (game: Game) => {
    setEditingGameId(game.id);
    setSavePathDraft(
      game.save_path ?? pathsByGameId[game.id]?.data?.save_path ?? "",
    );
  };

  const cancelEdit = () => {
    setEditingGameId(null);
    setSavePathDraft("");
  };

  const selectSaveFolder = async () => {
    const selected = await pickDirectoryPath({
      title: "\u0412\u044b\u0431\u0440\u0430\u0442\u044c \u043f\u0430\u043f\u043a\u0443 \u0441 \u0441\u043e\u0445\u0440\u0430\u043d\u0435\u043d\u0438\u044f\u043c\u0438",
    });
    if (selected) setSavePathDraft(selected);
  };

  const selectSaveFile = async () => {
    const selected = await pickFilePath({
      title: "\u0412\u044b\u0431\u0440\u0430\u0442\u044c \u0444\u0430\u0439\u043b \u0441\u043e\u0445\u0440\u0430\u043d\u0435\u043d\u0438\u044f",
    });
    if (selected) setSavePathDraft(selected);
  };

  const insertGamePathToken = () => {
    const current = savePathDraft.trim();
    if (current.startsWith(GAME_PATH_TOKEN)) return;
    const needsSeparator =
      current.length > 0 &&
      !current.startsWith("\\") &&
      !current.startsWith("/");
    setSavePathDraft(
      `${GAME_PATH_TOKEN}${needsSeparator ? "\\" : ""}${current}`,
    );
  };

  const saveGameSavePath = async (game: Game) => {
    if (savingSavePath) return;
    setSavingSavePath(true);
    try {
      const trimmed = savePathDraft.trim();
      const value = trimmed.length ? trimmed : null;
      await updateGame(game.id, { save_path: value });
      notify({ tone: "success", title: "Путь сохранён" });
      cancelEdit();
    } catch (e) {
      console.error("Failed to save save_path:", e);
      notify({
        tone: "error",
        title: "Не удалось сохранить путь",
        description: e instanceof Error ? e.message : "Неизвестная ошибка",
      });
    } finally {
      setSavingSavePath(false);
    }
  };

  const runScanAll = async () => {
    if (scanAll.loading) return;
    setScanAll({ loading: true });
    try {
      for (const game of filteredGames) {
        // Skip if already loaded.
        if (pathsByGameId[game.id]?.data) continue;
        // eslint-disable-next-line no-await-in-loop
        await loadSavePaths(game);
      }
    } finally {
      setScanAll({ loading: false, data: null });
    }
  };

  const handleRefreshManifest = async () => {
    try {
      await refreshSqobaManifest();
      notify({ tone: "success", title: "Манифест обновлён" });
      await refreshGames();
    } catch (e) {
      console.error("Failed to refresh SQOBA manifest:", e);
      notify({
        tone: "error",
        title: "Не удалось обновить манифест",
        description: e instanceof Error ? e.message : "Неизвестная ошибка",
      });
    }
  };

  const aboutCards = useMemo(
    () => [
      {
        title: "Автопоиск сохранений",
        description:
          "SQOBA использует манифест (Ludusavi/PCGamingWiki), чтобы находить сейвы без ручной настройки.",
        icon: <Search className="h-5 w-5 text-emerald-300" />,
        className:
          "bg-gradient-to-b from-emerald-500/28 via-emerald-500/8 to-background/20 border-emerald-400/25 shadow-[0_30px_90px_rgba(16,185,129,0.18)]",
      },
      {
        title: "Файлы и папки",
        description:
          "Можно бэкапить как целую папку, так и конкретный файл (например re2_config.ini).",
        icon: <FileIcon className="h-5 w-5 text-sky-300" />,
        className:
          "bg-gradient-to-b from-sky-500/26 via-indigo-500/10 to-background/20 border-sky-400/25 shadow-[0_30px_90px_rgba(56,189,248,0.16)]",
      },
      {
        title: "Шаблон {PATHTOGAME}",
        description:
          "Путь можно хранить относительно папки игры, чтобы переносы не ломали бэкапы.",
        icon: <Sparkles className="h-5 w-5 text-amber-300" />,
        className:
          "bg-gradient-to-b from-amber-500/28 via-orange-500/10 to-background/20 border-amber-400/25 shadow-[0_30px_90px_rgba(245,158,11,0.16)]",
      },
      {
        title: "История и откат",
        description:
          "Каждый бэкап хранит манифест файлов — можно спокойно откатываться на нужную дату.",
        icon: <Save className="h-5 w-5 text-fuchsia-300" />,
        className:
          "bg-gradient-to-b from-rose-500/22 via-fuchsia-500/10 to-background/20 border-rose-400/25 shadow-[0_30px_90px_rgba(244,63,94,0.14)]",
      },
      {
        title: "Сжатие SQOBA",
        description:
          "Опционально сжимает бэкапы для экономии места, без потери совместимости.",
        icon: <Sparkles className="h-5 w-5 text-teal-300" />,
        className:
          "bg-gradient-to-b from-teal-500/26 via-cyan-500/10 to-background/20 border-teal-400/25 shadow-[0_30px_90px_rgba(20,184,166,0.14)]",
      },
      {
        title: "Оффлайн-режим",
        description:
          "Даже без интернета SQOBA использует встроенный снапшот манифеста и локальный кэш.",
        icon: <RefreshCw className="h-5 w-5 text-slate-300" />,
        className:
          "bg-gradient-to-b from-slate-500/18 via-zinc-500/10 to-background/20 border-white/12 shadow-[0_30px_90px_rgba(148,163,184,0.10)]",
      },
    ],
    [],
  );

  return (
    <div className="p-4 sm:p-6 max-w-6xl mx-auto">
      <div className="flex items-start justify-between gap-4 mb-6">
        <div className="min-w-0">
          <h1 className="text-xl sm:text-2xl font-bold tracking-tight">
            SQOBA
          </h1>
          <p className="text-muted-foreground text-xs sm:text-sm">
            Сейвы, манифест и настройки бэкапов — в одном месте
          </p>
        </div>

        <Button
          variant="outline"
          className={cn(
            "gap-2 shadow-[0_10px_30px_rgba(8,12,24,0.18)]",
            "bg-gradient-to-b from-background to-muted/30",
          )}
          onClick={() => setAboutOpen(true)}
        >
          <Sparkles className="h-4 w-4" />
          Что такое SQOBA?
        </Button>
      </div>

      <div className="flex flex-col gap-4">
        {/* Saves */}
        <section className="order-2 space-y-4">
          <div className="rounded-xl border bg-card/80 backdrop-blur-xl shadow-[0_18px_45px_rgba(8,12,24,0.12)]">
            <div className="p-4 border-b border-border/60 flex flex-wrap items-center gap-3">
              <div className="min-w-0">
                <div className="text-sm font-semibold">Сейвы</div>
                <div className="text-xs text-muted-foreground">
                  Игры: {games.length} • Без пути: {missingCount}
                </div>
              </div>

              <div className="ml-auto flex items-center gap-2">
                <div className="flex items-center gap-2 rounded-lg border bg-background/40 px-3 py-2">
                  <Search className="h-4 w-4 text-muted-foreground" />
                  <input
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    placeholder="Поиск игры…"
                    className="bg-transparent text-sm outline-none w-44 sm:w-56 placeholder:text-muted-foreground"
                  />
                </div>
                <div className="hidden sm:flex items-center gap-2 rounded-lg border bg-background/40 px-3 py-2">
                  <span className="text-xs text-muted-foreground">
                    Только без пути
                  </span>
                  <Switch
                    checked={onlyMissing}
                    onCheckedChange={setOnlyMissing}
                  />
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={runScanAll}
                  disabled={
                    scanAll.loading ||
                    gamesLoading ||
                    filteredGames.length === 0
                  }
                  className="gap-2"
                  title="Найти пути для всех игр в списке"
                >
                  {scanAll.loading ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Search className="h-4 w-4" />
                  )}
                  Найти всё
                </Button>
              </div>
            </div>

            <div className="p-4 space-y-3">
              {gamesLoading ? (
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Загрузка игр…
                </div>
              ) : filteredGames.length === 0 ? (
                <div className="text-sm text-muted-foreground">
                  Ничего не найдено.
                </div>
              ) : (
                filteredGames.map((game) => {
                  const pathLookup = pathsByGameId[game.id];
                  const filesLookup = filesByGameId[game.id];
                  const isEditing = editingGameId === game.id;
                  const shownPath =
                    game.save_path ?? pathLookup?.data?.save_path ?? null;

                  const fileInfo = filesLookup?.data ?? null;
                  const fileCount = fileInfo?.files?.length ?? 0;
                  const totalSize = fileInfo?.total_size ?? 0;

                  return (
                    <div
                      key={game.id}
                      className={cn(
                        "rounded-xl border bg-background/40 hover:bg-background/55 transition-colors",
                        "shadow-[0_10px_30px_rgba(8,12,24,0.08)]",
                      )}
                    >
                      <div className="p-3 flex items-start gap-3">
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2 min-w-0">
                            <div className="truncate font-medium">
                              {game.name}
                            </div>
                            {game.save_path ? (
                              <span className="text-[10px] rounded-full border border-emerald-500/25 bg-emerald-500/10 px-2 py-0.5 text-emerald-300">
                                вручную
                              </span>
                            ) : (
                              <span className="text-[10px] rounded-full border border-amber-500/25 bg-amber-500/10 px-2 py-0.5 text-amber-300">
                                авто
                              </span>
                            )}
                            {fileCount > 0 && (
                              <span className="text-[10px] rounded-full border border-border/60 bg-background/50 px-2 py-0.5 text-muted-foreground">
                                {fileCount} • {formatBytes(totalSize)}
                              </span>
                            )}
                          </div>

                          {shownPath ? (
                            <div className="mt-1 text-xs text-muted-foreground break-words">
                              {shownPath}
                            </div>
                          ) : (
                            <div className="mt-1 text-xs text-muted-foreground">
                              Путь не задан
                            </div>
                          )}
                        </div>

                        <div className="flex items-center gap-2 flex-shrink-0">
                          <Button
                            variant="outline"
                            size="icon"
                            onClick={() => loadSavePaths(game)}
                            disabled={pathLookup?.loading}
                            title="Найти путь"
                          >
                            {pathLookup?.loading ? (
                              <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                              <Search className="h-4 w-4" />
                            )}
                          </Button>

                          <Button
                            variant="outline"
                            size="icon"
                            onClick={() => loadSaveFiles(game)}
                            disabled={filesLookup?.loading}
                            title="Показать файлы"
                          >
                            {filesLookup?.loading ? (
                              <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                              <FileIcon className="h-4 w-4" />
                            )}
                          </Button>

                          <Button
                            variant="outline"
                            size="icon"
                            onClick={() =>
                              shownPath && openSavePath(game, shownPath)
                            }
                            disabled={!shownPath}
                            title="Открыть"
                          >
                            <ExternalLink className="h-4 w-4" />
                          </Button>

                          <Button
                            variant={isEditing ? "default" : "outline"}
                            size="icon"
                            onClick={() =>
                              isEditing ? cancelEdit() : beginEdit(game)
                            }
                            title={isEditing ? "Закрыть" : "Изменить путь"}
                          >
                            {isEditing ? (
                              <X className="h-4 w-4" />
                            ) : (
                              <Pencil className="h-4 w-4" />
                            )}
                          </Button>
                        </div>
                      </div>

                      {isEditing && (
                        <div className="px-3 pb-3">
                          <div className="rounded-xl border bg-background/50 p-3">
                            <div className="text-xs text-muted-foreground mb-2">
                              Укажите путь к сейвам (папка или файл). Можно
                              использовать {GAME_PATH_TOKEN}.
                            </div>

                            <Input
                              value={savePathDraft}
                              onChange={(e) => setSavePathDraft(e.target.value)}
                              placeholder="Например: {PATHTOGAME}\\saves или C:\\Users\\...\\Saved Games"
                              className="font-mono text-xs"
                            />

                            <div className="mt-2">
                              <PathPreview value={savePathDraft} />
                            </div>

                            <div className="mt-3 flex flex-wrap gap-2">
                              <Button
                                variant="outline"
                                size="sm"
                                className="gap-2"
                                onClick={selectSaveFolder}
                              >
                                <FolderOpen className="h-4 w-4" />
                                Папка
                              </Button>
                              <Button
                                variant="outline"
                                size="sm"
                                className="gap-2"
                                onClick={selectSaveFile}
                              >
                                <FileIcon className="h-4 w-4" />
                                Файл
                              </Button>
                              <Button
                                variant="outline"
                                size="sm"
                                className="gap-2"
                                onClick={insertGamePathToken}
                                title="Вставить шаблон в начало"
                              >
                                <Sparkles className="h-4 w-4" />
                                {GAME_PATH_TOKEN}
                              </Button>
                              <Button
                                variant="outline"
                                size="sm"
                                className="gap-2"
                                onClick={() =>
                                  savePathDraft.trim() &&
                                  openSavePath(game, savePathDraft.trim())
                                }
                                disabled={!savePathDraft.trim()}
                              >
                                <ExternalLink className="h-4 w-4" />
                                Открыть
                              </Button>

                              <div className="flex-1" />

                              <Button
                                size="sm"
                                className="gap-2"
                                onClick={() => saveGameSavePath(game)}
                                disabled={savingSavePath}
                              >
                                {savingSavePath ? (
                                  <Loader2 className="h-4 w-4 animate-spin" />
                                ) : (
                                  <Save className="h-4 w-4" />
                                )}
                                Сохранить
                              </Button>
                            </div>
                          </div>
                        </div>
                      )}

                      {filesLookup?.data?.files?.length ? (
                        <div className="px-3 pb-3">
                          <div className="rounded-xl border bg-background/50">
                            <div className="p-3 border-b border-border/60 flex items-center justify-between gap-3">
                              <div className="min-w-0">
                                <div className="text-sm font-semibold">
                                  Файлы сохранений
                                </div>
                                <div className="text-xs text-muted-foreground">
                                  {filesLookup.data.files.length} файлов •{" "}
                                  {formatBytes(filesLookup.data.total_size)}
                                </div>
                              </div>
                              {filesLookup.data.save_path ? (
                                <Button
                                  variant="outline"
                                  size="sm"
                                  className="gap-2"
                                  onClick={() =>
                                    filesLookup.data?.save_path &&
                                    openSavePath(
                                      game,
                                      filesLookup.data.save_path,
                                    )
                                  }
                                >
                                  <ExternalLink className="h-4 w-4" />
                                  Открыть папку
                                </Button>
                              ) : null}
                            </div>

                            <ScrollArea className="max-h-56">
                              <div className="p-3 space-y-1">
                                {filesLookup.data.files
                                  .slice(0, 200)
                                  .map((p) => (
                                    <div
                                      key={p}
                                      className="font-mono text-[11px] text-muted-foreground break-all"
                                    >
                                      {p}
                                    </div>
                                  ))}
                                {filesLookup.data.files.length > 200 ? (
                                  <div className="text-xs text-muted-foreground mt-2">
                                    Показаны первые 200 файлов.
                                  </div>
                                ) : null}
                              </div>
                            </ScrollArea>
                          </div>
                        </div>
                      ) : null}
                    </div>
                  );
                })
              )}
            </div>
          </div>
        </section>

        {/* Settings */}
        <section className="order-1 space-y-4">
          <div className="rounded-xl border bg-card/80 backdrop-blur-xl shadow-[0_18px_45px_rgba(8,12,24,0.12)] p-4">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-sm font-semibold">Манифест SQOBA</div>
                <div className="text-xs text-muted-foreground leading-relaxed">
                  Используется для автопоиска путей. Работает из кэша и
                  встроенного снапшота.
                </div>
              </div>
              <Button
                variant="outline"
                size="sm"
                className="gap-2"
                onClick={handleRefreshManifest}
              >
                <RefreshCw className="h-4 w-4" />
                Обновить
              </Button>
            </div>
          </div>

          <div className="rounded-xl border bg-card/80 backdrop-blur-xl shadow-[0_18px_45px_rgba(8,12,24,0.12)] overflow-hidden">
            <div className="p-4 border-b border-border/60">
              <div className="text-sm font-semibold">Настройки SQOBA</div>
              <div className="text-xs text-muted-foreground">
                Папка бэкапов, автодействия и сжатие.
              </div>
            </div>

            <div className="p-4 space-y-4">
              {settingsLoading ? (
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Загрузка настроек…
                </div>
              ) : (
                <>
                  <div className="space-y-2">
                    <div className="text-xs font-medium">Папка для бэкапов</div>
                    <div className="flex gap-2">
                      <Input
                        value={backupDirectory}
                        onChange={(e) => setBackupDirectory(e.target.value)}
                        placeholder="Путь к папке"
                        className="flex-1"
                      />
                      <Button
                        variant="outline"
                        size="icon"
                        onClick={selectBackupDirectory}
                        title="Выбрать папку"
                      >
                        <FolderOpen className="h-4 w-4" />
                      </Button>
                    </div>
                  </div>

                  <div className="rounded-xl border bg-background/40 p-3 space-y-3">
                    <div className="flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="text-sm font-medium">
                          Автоматические бэкапы
                        </div>
                        <div className="text-xs text-muted-foreground">
                          Создавать бэкап при выходе из игры (если включено).
                        </div>
                      </div>
                      <Switch
                        checked={autoBackup}
                        onCheckedChange={setAutoBackup}
                      />
                    </div>

                    <div className="flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="text-sm font-medium">
                          Предлагать бэкап перед запуском
                        </div>
                        <div className="text-xs text-muted-foreground">
                          Если сейвы изменились — предложить сделать бэкап.
                        </div>
                      </div>
                      <Switch
                        checked={backupBeforeLaunch}
                        onCheckedChange={setBackupBeforeLaunch}
                      />
                    </div>
                  </div>

                  <div className="rounded-xl border bg-background/40 p-3 space-y-3">
                    <div className="flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="text-sm font-medium">Сжатие SQOBA</div>
                        <div className="text-xs text-muted-foreground">
                          Меньше размер, чуть дольше создание.
                        </div>
                      </div>
                      <Switch
                        checked={compressionEnabled}
                        onCheckedChange={(v) =>
                          handleCompressionToggle(Boolean(v))
                        }
                      />
                    </div>

                    <div className="space-y-2">
                      <div className="flex items-center justify-between text-xs text-muted-foreground">
                        <span>Уровень</span>
                        <span className="tabular-nums">{compressionLevel}</span>
                      </div>
                      <input
                        type="range"
                        min={1}
                        max={100}
                        value={compressionLevel}
                        onChange={(e) =>
                          handleCompressionLevelChange(
                            parseInt(e.target.value, 10),
                          )
                        }
                        className="w-full accent-foreground"
                        disabled={!compressionEnabled}
                      />
                    </div>
                  </div>

                  <div className="space-y-2">
                    <div className="text-xs font-medium">
                      Макс. кол-во бэкапов на игру
                    </div>
                    <Input
                      type="number"
                      min={1}
                      max={100}
                      value={maxBackups}
                      onChange={(event) =>
                        handleMaxBackupsChange(parseInt(event.target.value, 10))
                      }
                      className="w-28"
                    />
                  </div>

                  <div className="flex justify-end">
                    <Button
                      onClick={saveSettings}
                      disabled={settingsSaving}
                      className="gap-2"
                    >
                      {settingsSaving ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <Save className="h-4 w-4" />
                      )}
                      Сохранить
                    </Button>
                  </div>
                </>
              )}
            </div>
          </div>
        </section>
      </div>

      {/* ABOUT SQOBA */}
      {aboutOpen && (
        <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm p-4 flex items-center justify-center animate-in fade-in duration-200 relative">
          <button
            type="button"
            aria-label="Close"
            className="absolute inset-0"
            onMouseDown={() => setAboutOpen(false)}
          />
          <div className="w-full max-w-5xl max-h-[90vh] rounded-2xl border bg-background/80 shadow-[0_30px_80px_rgba(0,0,0,0.55)] overflow-hidden flex flex-col relative z-10">
            <div className="p-5 border-b border-border/60 flex items-start justify-between gap-4">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <div className="h-9 w-9 rounded-xl border border-white/10 bg-gradient-to-br from-emerald-500/20 via-sky-500/10 to-transparent shadow-[0_10px_30px_rgba(8,12,24,0.25)] flex items-center justify-center">
                    <Sparkles className="h-4 w-4 text-foreground" />
                  </div>
                  <div>
                    <div className="text-lg font-bold tracking-tight">
                      SQOBA
                    </div>
                    <div className="text-xs text-muted-foreground">
                      Коротко и по делу — что он умеет
                    </div>
                  </div>
                </div>
              </div>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setAboutOpen(false)}
              >
                <X className="h-5 w-5" />
              </Button>
            </div>

            <ScrollArea className="flex-1">
              <div className="p-5">
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
                  {aboutCards.map((card, index) => (
                    <div
                      key={card.title}
                      className="animate-in fade-in slide-in-from-bottom-2 duration-300"
                      style={{ animationDelay: `${index * 60}ms` }}
                    >
                      <InfoCard
                        title={card.title}
                        description={card.description}
                        icon={card.icon}
                        className={card.className}
                      />
                    </div>
                  ))}
                </div>

                <div className="mt-5 rounded-xl border bg-muted/30 p-4">
                  <div className="text-sm font-semibold">Быстрый совет</div>
                  <div className="text-xs text-muted-foreground mt-1 leading-relaxed">
                    Если игра переехала в другую папку — храните путь к сейвам
                    через <span className="font-mono">{GAME_PATH_TOKEN}</span>.
                    Тогда достаточно обновить путь к exe, и бэкапы продолжат
                    работать.
                  </div>
                </div>
              </div>
            </ScrollArea>
          </div>
        </div>
      )}
    </div>
  );
}
