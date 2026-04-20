import {
  Activity,
  ArrowLeft,
  Download,
  File as FileIcon,
  FolderOpen,
  Gamepad2,
  ExternalLink,
  Globe,
  HardDrive,
  Image as ImageIcon,
  Check,
  Loader2,
  Flag,
  ShieldCheck,
  XCircle,
  Pencil,
  Play,
  Save,
  Search,
  Settings,
  Star,
  Timer,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import type { MouseEvent as ReactMouseEvent } from "react";
import { gamePosterCardClasses } from "../../../../packages/kepler-visuals/patterns";
import { useToast } from "@/components/ToastProvider";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Switch } from "@/components/ui/switch";
import { useGameStatus } from "@/hooks/useGameStatus";
import {
  openPath,
  pickDirectoryPath,
  pickFilePath,
  subscribeAppEvent,
} from "@/lib/browser";
import { backupApi, gamesApi, metadataApi } from "@/lib/api";
import { translateGenreListToRu } from "@/lib/genres";
import { cn } from "@/lib/utils";
import { useGamesActions, useGamesState } from "@/store/GamesContext";
import type {
  Backup,
  Game,
  RawgGame,
  RestoreCheck,
} from "@/types";

type BackupProgressPayload = {
  game_id: string;
  stage: string;
  message: string;
  done: number;
  total: number;
};

function formatPlaytime(seconds: number) {
  if (!seconds) return "0 ч";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) return `${hours} ч ${minutes} мин`;
  return `${minutes} мин`;
}

function normalizeDescription(value: string | null) {
  if (!value) return null;
  const plain = value
    .replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  if (!plain) return null;
  return plain;
}

function formatPlayedHours(seconds: number) {
  if (seconds <= 0) return "0 ч";
  if (seconds < 3600) return "<1 ч";
  return `${Math.floor(seconds / 3600)} ч`;
}

export default function GameDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { games } = useGamesState();
  const { toggleFavorite, deleteGame, refreshGames } = useGamesActions();

  const [game, setGame] = useState<Game | null>(null);

  // Edit Dialog State
  const [showEditDialog, setShowEditDialog] = useState(false);
  const [editForm, setEditForm] = useState<{
    name: string;
    description: string;
    background_image: string;
    cover_image: string;
  }>({
    name: "",
    description: "",
    background_image: "",
    cover_image: "",
  });

  const [saving, setSaving] = useState(false);
  const [launching, setLaunching] = useState(false);

  // Metadata search
  const [showMetadataSearch, setShowMetadataSearch] = useState(false);
  const [metadataQuery, setMetadataQuery] = useState("");
  const [metadataResults, setMetadataResults] = useState<RawgGame[]>([]);
  const [searchingMetadata, setSearchingMetadata] = useState(false);
  const [applyingMetadata, setApplyingMetadata] = useState(false);

  const [renameFromMetadata, setRenameFromMetadata] = useState(false);

  // Backups
  const [backups, setBackups] = useState<Backup[]>([]);
  const [loadingBackups, setLoadingBackups] = useState(false);
  const [, setCreatingBackup] = useState(false);
  const [showBackupPrompt, setShowBackupPrompt] = useState(false);
  const [savePathDraft, setSavePathDraft] = useState("");
  const [savingSavePath, setSavingSavePath] = useState(false);
  const [locatingSavePath, setLocatingSavePath] = useState(false);

  const [showAllBackups, setShowAllBackups] = useState(false);
  const [showRestorePrompt, setShowRestorePrompt] = useState(false);
  const [restoreInfo, setRestoreInfo] = useState<RestoreCheck | null>(null);
  const [restoring, setRestoring] = useState(false);

  const [backupProgress, setBackupProgress] = useState({
    active: false,
    stage: "",
    message: "",
    done: 0,
    total: 0,
  });
  const {
    isInstalled,
    checkingInstalled,
    runningCount,
    checkingRunning,
    setRunningCount,
  } = useGameStatus(game?.id, game?.exe_path);
  const [userRating, setUserRating] = useState<number | null>(null);
  const [userNote, setUserNote] = useState("");
  const [playStatus, setPlayStatus] = useState<Game["play_status"]>("not_started");
  const [savingUserRating, setSavingUserRating] = useState(false);
  const [savingUserNote, setSavingUserNote] = useState(false);
  const [showRatingModal, setShowRatingModal] = useState(false);
  const [ratingDraft, setRatingDraft] = useState(4);
  const [showGameSettings, setShowGameSettings] = useState(false);
  const [showDescriptionModal, setShowDescriptionModal] = useState(false);

  const { notify } = useToast();

  const latestBackup = backups[0];
  const olderBackups = backups.slice(1);
  const gamePathToken = "{PATHTOGAME}";
  const savePathValue = savePathDraft.trim();
  const savedPath = game?.save_path ?? "";
  const savePathDirty = savePathValue !== savedPath;
  const playStatusLabel: Record<Game["play_status"], string> = {
    not_started: "Не начато",
    in_progress: "В процессе",
    completed: "Пройдено",
    abandoned: "Брошено",
  };
  const playStatusIcon: Record<Game["play_status"], typeof Check> = {
    not_started: XCircle,
    in_progress: Flag,
    completed: ShieldCheck,
    abandoned: Check,
  };
  const playStatusClasses: Record<Game["play_status"], string> = {
    not_started: "text-muted-foreground border-muted-foreground/35",
    in_progress: "text-sky-400 border-sky-400/45",
    completed: "text-emerald-400 border-emerald-400/45",
    abandoned: "text-rose-400 border-rose-400/45",
  };
  const ratingLevels = [
    { value: 1, label: "1", desc: "Ужасно, играть невозможно" },
    { value: 2, label: "2", desc: "Плохо, много проблем" },
    { value: 3, label: "3", desc: "Слабо, на один раз" },
    { value: 4, label: "4", desc: "Нормально, без восторга" },
    { value: 5, label: "5", desc: "Хорошо, понравилось" },
    { value: 6, label: "6", desc: "Отлично, рекомендую" },
    { value: 7, label: "7", desc: "Шедевр, топ" },
  ];
  void ratingLevels;

  const displayRating = showRatingModal ? ratingDraft : userRating;
  const isRunning = runningCount > 0;
  const isMissing = !isInstalled && !checkingInstalled;
  const heroImage = game?.background_image || game?.cover_image || null;
  const heroGenres = translateGenreListToRu(game?.genres, 3).join(" · ") || null;
  const heroDescription = normalizeDescription(game?.description ?? null);
  const heroReleaseYear = game?.released
    ? new Date(game.released).getFullYear().toString()
    : null;
  const heroMeta = [heroReleaseYear, formatPlayedHours(game?.total_playtime ?? 0)]
    .filter(Boolean)
    .join(" · ");
  const playState = isMissing ? "missing" : isRunning ? "running" : "ready";
  const playLabel = isMissing
    ? "Не установлена"
    : launching
      ? "Запускается"
      : isRunning
        ? "Запущена"
        : "Играть";

  useEffect(() => {
    const found = games.find((g) => g.id === id);
    if (found) {
      const isSameGame = game?.id === found.id;
      const currentSavedPath = game?.save_path ?? "";
      const draftTrimmed = savePathDraft.trim();
      setGame(found);
      // Initialize edit form
      setEditForm({
        name: found.name,
        description: found.description || "",
        background_image: found.background_image || "",
        cover_image: found.cover_image || "",
      });
      setPlayStatus(found.play_status || "not_started");
      setUserRating(found.user_rating ?? null);
      setUserNote(found.user_note || "");
      if (!isSameGame || draftTrimmed === currentSavedPath) {
        setSavePathDraft(found.save_path || "");
      }
    }
  }, [id, games]);

  useEffect(() => {
    if (game) {
      loadBackups();
    }
  }, [game?.id]);

  useEffect(() => {
    if (!showDescriptionModal) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setShowDescriptionModal(false);
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [showDescriptionModal]);

  useEffect(() => {
    if (!game) return;
    const unlistenBackup = subscribeAppEvent<BackupProgressPayload>(
      "backup:progress",
      (payload) => {
        if (payload.game_id !== game.id) return;
        const { stage, message, done, total } = payload;
        if (stage === "done") {
          setBackupProgress({ active: false, stage, message, done, total });
        } else {
          setBackupProgress({ active: true, stage, message, done, total });
        }
      },
    );
    const unlistenRestore = subscribeAppEvent<BackupProgressPayload>(
      "restore:progress",
      (payload) => {
        if (payload.game_id !== game.id) return;
        const { stage, message, done, total } = payload;
        if (stage === "done") {
          setBackupProgress({ active: false, stage, message, done, total });
        } else {
          setBackupProgress({ active: true, stage, message, done, total });
        }
      },
    );
    return () => {
      unlistenBackup();
      unlistenRestore();
    };
  }, [game?.id]);

  const loadBackups = async () => {
    if (!game) return;
    setLoadingBackups(true);
    try {
      const data = await backupApi.getForGame(game.id);
      setBackups(data);
    } catch (e) {
      console.error("Failed to load backups:", e);
    } finally {
      setLoadingBackups(false);
    }
  };

  const handleLaunch = async () => {
    if (!game || isMissing) return;

    if (runningCount > 0) {
      const label =
        runningCount > 1
          ? `Закрыть ${runningCount} процесса?`
          : "Закрыть игру?";
      if (!confirm(`Игра уже запущена. ${label}`)) return;
      try {
        await gamesApi.killProcesses(game.id);
        const remaining = await gamesApi.getRunningInstances(game.id);
        setRunningCount(remaining);
        if (remaining > 0) {
          alert(
            `Не удалось завершить все процессы. Осталось: ${remaining}.`,
          );
        }
      } catch (e) {
        console.error("Failed to kill processes:", e);
        alert(`Не удалось закрыть игру: ${(e as Error).message}`);
      }
      return;
    }

    setLaunching(true);
    try {
      if (game.backup_enabled) {
        const restoreCheck = await backupApi.checkRestoreNeeded(
          game.id,
          game.name,
        );
        if (restoreCheck.should_restore && restoreCheck.backup_id) {
          setRestoreInfo(restoreCheck);
          setShowRestorePrompt(true);
          setLaunching(false);
          return;
        }

        const shouldBackup = await backupApi.shouldBackupBeforeLaunch(game.id);
        if (shouldBackup) {
          const needsBackup = await backupApi.checkBackupNeeded(
            game.id,
            game.name,
          );
          if (needsBackup) {
            setShowBackupPrompt(true);
            setLaunching(false);
            return;
          }
        }
      }

      await launchGame();
    } catch (e) {
      console.error("Launch error:", e);
      setLaunching(false);
    }
  };

  const launchGame = async () => {
    if (!game) return;
    try {
      // Launch and track time
      await gamesApi.launch(game.id);
      const count = await gamesApi.getRunningInstances(game.id);
      setRunningCount(count);
      await refreshGames();
    } catch (e) {
      console.error("Failed to launch:", e);
    } finally {
      setLaunching(false);
    }
  };

  const startBackupInBackground = (isAuto: boolean) => {
    if (!game) return;
    setCreatingBackup(true);
    backupApi
      .create(game.id, game.name, isAuto)
      .then(async () => {
        await loadBackups();
        await refreshGames();
      })
      .catch((e) => {
        console.error("Backup failed:", e);
        alert(`Ошибка бэкапа: ${(e as Error).message}`);
      })
      .finally(() => {
        setCreatingBackup(false);
      });
  };

  const createBackupAndRefresh = async (isAuto: boolean) => {
    if (!game) return;
    await backupApi.create(game.id, game.name, isAuto);
    await loadBackups();
    await refreshGames();
  };

  const handleBackupAndLaunch = async () => {
    if (!game) return;
    setShowBackupPrompt(false);
    setLaunching(true);
    try {
      await createBackupAndRefresh(true);
      await launchGame();
    } catch (e) {
      console.error("Backup before launch failed:", e);
      alert(`Ошибка бэкапа: ${(e as Error).message}`);
      setLaunching(false);
    }
  };

  const handleRestoreAndLaunch = async () => {
    if (!game || !restoreInfo?.backup_id) return;
    const backupId = restoreInfo.backup_id;
    setShowRestorePrompt(false);
    setRestoreInfo(null);
    setRestoring(true);
    try {
      await backupApi.restore(backupId);
      await refreshGames();
      setLaunching(true);
      await launchGame();
    } catch (e) {
      console.error("Restore failed:", e);
      alert(`Ошибка восстановления: ${(e as Error).message}`);
    } finally {
      setRestoring(false);
    }
  };

  const handleSkipRestore = async () => {
    setShowRestorePrompt(false);
    setRestoreInfo(null);
    setLaunching(true);
    await launchGame();
  };

  const handleToggleFavorite = async () => {
    if (!game) return;
    await toggleFavorite(game.id);
  };

  const handleDelete = async () => {
    if (!game) return;
    if (confirm(`Удалить "${game.name}" из библиотеки?`)) {
      await deleteGame(game.id);
      navigate("/");
    }
  };

  const handleSaveEdit = async () => {
    if (!game) return;
    setSaving(true);
    try {
      await gamesApi.update({
        id: game.id,
        name: editForm.name,
        description: editForm.description,
        background_image: editForm.background_image,
        cover_image: editForm.cover_image,
      });
      await refreshGames();
      setShowEditDialog(false);
    } catch (e) {
      console.error("Failed to update:", e);
    } finally {
      setSaving(false);
    }
  };

  const handleSearchGoogleImage = async (
    searchQueryText: string,
    target: "background" | "cover" = "background",
  ) => {
    const targetSuffix = target === "cover" ? " cover" : " background";
    const query = encodeURIComponent(`${searchQueryText}${targetSuffix}`);
    const url = `https://www.google.com/search?tbm=isch&q=${query}`;
    await openPath(url);
  };

  const searchMetadata = async () => {
    if (!metadataQuery.trim()) return;
    setSearchingMetadata(true);
    try {
      const results = await metadataApi.search(metadataQuery);
      setMetadataResults(results);
    } catch (e) {
      console.error("Metadata search failed:", e);
    } finally {
      setSearchingMetadata(false);
    }
  };

  const applyMetadata = async (rawgGame: RawgGame) => {
    if (!game) return;
    setApplyingMetadata(true);
    try {
      await metadataApi.apply(game.id, rawgGame.id, renameFromMetadata);
      await refreshGames();
      setShowMetadataSearch(false);
      setMetadataResults([]);
      setMetadataQuery("");
    } catch (e) {
      console.error("Failed to apply metadata:", e);
    } finally {
      setApplyingMetadata(false);
    }
  };

  const createManualBackup = async () => {
    if (!game) return;
    startBackupInBackground(false);
  };

  const restoreBackup = async (backupId: string, withConfirm = true) => {
    if (
      withConfirm &&
      !confirm("Восстановить бэкап? Текущие сохранения будут перезаписаны.")
    )
      return;
    try {
      await backupApi.restore(backupId);
      alert("Бэкап успешно восстановлен!");
    } catch (e) {
      console.error("Restore failed:", e);
      alert(`Ошибка восстановления: ${(e as Error).message}`);
    }
  };

  const toggleBackupEnabled = async (next?: boolean) => {
    if (!game) return;
    const desired = typeof next === "boolean" ? next : !game.backup_enabled;
    if (desired === game.backup_enabled) return;
    try {
      await gamesApi.update({
        id: game.id,
        backup_enabled: desired,
      });
      await refreshGames();
    } catch (e) {
      console.error("Failed to update backup setting:", e);
    }
  };

  const handleSelectSavePath = async () => {
    const selected = await pickDirectoryPath({
      title: "Выбрать папку с сохранениями",
    });

    if (selected) {
      setSavePathDraft(selected);
    }
  };

  const handleSelectSaveFile = async () => {
    const selected = await pickFilePath({
      title: "Выбрать файл сохранения",
    });

    if (selected) {
      setSavePathDraft(selected);
    }
  };

  const resolveSavePathTemplate = (path: string) => {
    if (!path.includes("{PATHTOGAME}")) return path;
    const exePath = game?.exe_path;
    if (!exePath) return path;
    const lastSlash = Math.max(
      exePath.lastIndexOf("\\"),
      exePath.lastIndexOf("/"),
    );
    if (lastSlash <= 0) {
      return path.replace(/{PATHTOGAME}/g, exePath);
    }
    const base = exePath.slice(0, lastSlash);
    return path.replace(/{PATHTOGAME}/g, base);
  };

  const handleOpenSavePath = async () => {
    if (!savePathValue) return;
    try {
      await openPath(resolveSavePathTemplate(savePathValue));
    } catch (e) {
      console.error("Failed to open save path:", e);
    }
  };

  const handleLocateSavePath = async () => {
    if (!game) return;
    setLocatingSavePath(true);
    try {
      const info = await backupApi.findGameSavePaths(game.name, game.id);
      if (info.save_path) {
        setSavePathDraft(info.save_path);
        if (info.candidates.length > 1) {
          notify({
            tone: "info",
            title: "Найдено несколько вариантов",
            description:
              "Проверьте, что выбранный путь действительно содержит сохранения.",
          });
        }
      } else {
        notify({
          tone: "warning",
          title: "Сохранения не найдены",
          description:
            "Укажите путь вручную, чтобы сохранения попадали в бэкапы.",
        });
      }
    } catch (e) {
      console.error("Failed to locate saves:", e);
      notify({
        tone: "error",
        title: "Ошибка поиска сохранений",
        description: "Не удалось проверить путь сохранений.",
      });
    } finally {
      setLocatingSavePath(false);
    }
  };

  const handleInsertGamePathToken = () => {
    const current = savePathDraft.trim();
    if (current.startsWith(gamePathToken)) return;
    const needsSeparator =
      current.length > 0 &&
      !current.startsWith("\\") &&
      !current.startsWith("/");
    const next = `${gamePathToken}${needsSeparator ? "\\" : ""}${current}`;
    setSavePathDraft(next);
  };

  const renderSavePathPreview = () => {
    if (!savePathValue.includes(gamePathToken)) return null;
    const parts = savePathValue.split(gamePathToken);
    return (
      <div className="text-[11px] text-muted-foreground">
        {"Путь: "}
        {parts.map((part, index) => (
          <span key={`${index}-${part}`}>
            {part}
            {index < parts.length - 1 && (
              <span className="mx-1 rounded-md border border-emerald-500/40 bg-emerald-500/10 px-1 py-0.5 font-mono text-[10px] text-emerald-200">
                {gamePathToken}
              </span>
            )}
          </span>
        ))}
      </div>
    );
  };

  const handleSaveSavePath = async () => {
    if (!game) return;
    setSavingSavePath(true);
    try {
      const normalizedPath = savePathValue;
      await gamesApi.update({
        id: game.id,
        save_path: normalizedPath === "" ? null : normalizedPath,
      });
      setSavePathDraft(normalizedPath);
      await refreshGames();
    } catch (e) {
      console.error("Failed to update save path:", e);
    } finally {
      setSavingSavePath(false);
    }
  };

  const handleSavePlayStatus = async (nextStatus: Game["play_status"]) => {
    if (!game) return;
    try {
      await gamesApi.update({
        id: game.id,
        play_status: nextStatus,
      });
      setPlayStatus(nextStatus);
      await refreshGames();
    } catch (e) {
      console.error("Failed to update play status:", e);
    }
  };

  const handleSaveUserRating = async (nextRating?: number | null) => {
    if (!game) return;
    setSavingUserRating(true);
    try {
      const ratingValue = nextRating !== undefined ? nextRating : userRating;
      await gamesApi.update({
        id: game.id,
        user_rating: ratingValue,
        user_note: userNote,
      });
      if (nextRating !== undefined) {
        setUserRating(nextRating);
      }
      await refreshGames();
    } catch (e) {
      console.error("Failed to update user rating:", e);
    } finally {
      setSavingUserRating(false);
    }
  };

  const handleSaveUserNote = async () => {
    if (!game) return;
    setSavingUserNote(true);
    try {
      await gamesApi.update({
        id: game.id,
        user_note: userNote,
      });
      await refreshGames();
      notify({
        tone: "success",
        title: "Заметка сохранена",
      });
    } catch (e) {
      console.error("Failed to update user note:", e);
      notify({
        tone: "error",
        title: "Не удалось сохранить заметку",
      });
    } finally {
      setSavingUserNote(false);
    }
  };

  if (!game) {
    return (
      <div className="p-6">
        <p className="text-muted-foreground">Игра не найдена</p>
        <Link to="/" className="text-primary hover:underline">
          Назад в библиотеку
        </Link>
      </div>
    );
  }

  return (
    <div className="min-h-screen relative overflow-hidden">
      <div className="pointer-events-none absolute inset-0">
        <div
          className="absolute -top-48 right-[-160px] h-96 w-96 rounded-full opacity-60 blur-3xl"
          style={{
            background:
              "radial-gradient(circle, rgba(99, 102, 241, 0.55), rgba(99, 102, 241, 0))",
          }}
        />
        <div
          className="absolute top-24 left-[-180px] h-80 w-80 rounded-full opacity-40 blur-3xl"
          style={{
            background:
              "radial-gradient(circle, rgba(255, 255, 255, 0.12), rgba(255, 255, 255, 0))",
          }}
        />
      </div>
      {/* Hero Section */}
      <div
        data-testid="game-detail-hero"
        className="relative h-[80vh] min-h-[420px] max-h-[880px] overflow-hidden rounded-b-[32px] group sm:rounded-b-[40px]"
      >
        {heroImage ? (
          <img
            src={heroImage}
            alt={game.name}
            className="absolute inset-0 w-full h-full object-cover"
          />
        ) : (
          <div className="absolute inset-0 bg-gradient-to-br from-secondary to-muted" />
        )}
        <div className="absolute inset-0 bg-gradient-to-t from-background via-background/70 to-transparent" />
        <div className="absolute inset-0 pointer-events-none">
          <div
            className="absolute inset-0 opacity-70"
            style={{
              backgroundImage:
                "radial-gradient(60% 80% at 70% 10%, rgba(129, 140, 248, 0.45), transparent 70%)",
            }}
          />
          <div
            className="absolute inset-0 opacity-60"
            style={{
              backgroundImage:
                "radial-gradient(35% 45% at 20% 20%, rgba(244, 114, 182, 0.35), transparent 60%)",
            }}
          />
          <div
            className="absolute -top-8 left-1/2 h-24 w-24 -translate-x-1/2 rounded-full opacity-80 blur-2xl"
            style={{
              background:
                "radial-gradient(circle, rgba(255,255,255,0.8), rgba(255,255,255,0))",
            }}
          />
        </div>
        <div className="absolute inset-x-0 bottom-0 z-30 px-6 pb-8 sm:px-8 sm:pb-10 lg:px-10 lg:pb-12">
          <div className="flex flex-wrap items-end justify-between gap-4 lg:gap-6">
            <div
              data-testid="game-detail-hero-copy"
              className={cn(
                gamePosterCardClasses.content,
                "pointer-events-auto static inset-auto min-w-0 flex-1 gap-3 p-0",
              )}
            >
              <h1
                className={cn(
                  gamePosterCardClasses.title,
                  "max-w-[min(18ch,100%)] text-4xl font-semibold leading-[0.92] text-white drop-shadow-[0_10px_32px_rgba(0,0,0,0.45)] sm:text-5xl lg:text-6xl",
                )}
              >
                {game.name}
              </h1>
              {heroGenres ? (
                <p
                  className={cn(
                    gamePosterCardClasses.eyebrow,
                    "max-w-[min(70ch,100%)] text-white/80 text-sm font-medium sm:text-base",
                  )}
                >
                  {heroGenres}
                </p>
              ) : null}
              {heroDescription ? (
                <div className="flex max-w-[400px] items-end justify-between gap-3">
                  <p
                    data-testid="game-detail-hero-description"
                    className="truncate-2 min-w-0 flex-1 text-sm leading-6 text-white/72 sm:text-[15px]"
                  >
                    {heroDescription}
                  </p>
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    data-testid="game-detail-description-button"
                    className="h-auto shrink-0 self-end rounded-full border border-white/10 bg-background/50 px-2.5 py-1 text-xs text-white hover:bg-background/65 sm:text-[13px]"
                    onClick={() => setShowDescriptionModal(true)}
                  >
                    Еще
                  </Button>
                </div>
              ) : null}
              <p className="text-xs font-medium uppercase tracking-[0.16em] text-white/58 sm:text-[13px]">
                {heroMeta}
              </p>
            </div>
            <div
              data-testid="game-detail-hero-actions"
              className="flex max-w-[320px] flex-col items-end gap-2 self-end"
            >
              <Button
                size="lg"
                className={cn(
                  "h-14 gap-2 rounded-full px-8 text-lg shadow-none transition-all hover:shadow-none",
                  playState === "running" &&
                    "border border-white/10 bg-gradient-to-br from-rose-500/90 via-red-500/85 to-orange-500/80 text-white hover:text-white",
                  playState === "ready" &&
                    "border border-foreground/20 bg-foreground text-background hover:border-accent hover:bg-accent hover:text-white",
                  playState === "missing" &&
                    "bg-muted text-muted-foreground border border-border/60 shadow-none disabled:opacity-100",
                )}
                onClick={handleLaunch}
                disabled={isMissing || restoring || (launching && !isRunning)}
              >
                {isMissing ? (
                  <HardDrive className="w-5 h-5 opacity-70" />
                ) : isRunning ? (
                  <Activity className="w-5 h-5 animate-pulse" />
                ) : launching || checkingRunning ? (
                  <Loader2 className="w-5 h-5 animate-spin" />
                ) : (
                  <Play className="w-5 h-5 fill-current" />
                )}
                {playLabel}
              </Button>
              {isMissing ? (
                <div className="max-w-[280px] text-right text-xs text-white/72">
                  {
                    "Игра не установлена, но карточка остаётся в библиотеке — как IMDb для своих игр. Можно снова скачать, смотреть статистику и вернуться к бэкапам."
                  }
                </div>
              ) : null}
            </div>
          </div>
        </div>

        {/* Back button */}
        <div className="absolute top-4 left-4">
          <Button
            variant="secondary"
            size="icon"
            onClick={() => navigate(-1)}
            className="bg-background/60 backdrop-blur-md border border-white/10"
          >
            <ArrowLeft className="w-4 h-4" />
          </Button>
        </div>

        {/* Actions */}
        <div className="absolute top-4 right-4 flex items-center gap-2">
          <Button
            variant="secondary"
            size="icon"
            onClick={handleToggleFavorite}
            className="bg-background/60 backdrop-blur-md border border-white/10"
            title={game.is_favorite ? "Убрать из избранного" : "В избранное"}
          >
            <Star
              className={cn(
                "w-4 h-4",
                game.is_favorite && "fill-yellow-500 text-yellow-500",
              )}
            />
          </Button>
          <Button
            variant="secondary"
            size="icon"
            onClick={() => {
              setMetadataQuery(game.name);
              setShowMetadataSearch(true);
            }}
            className="bg-background/60 backdrop-blur-md border border-white/10"
            title="Найти метаданные (RAWG)"
          >
            <Search className="w-4 h-4" />
          </Button>
          <Button
            variant="secondary"
            size="icon"
            onClick={() => setShowGameSettings(true)}
            className="bg-background/60 backdrop-blur-md border border-white/10"
            title="Настройки игры"
          >
            <Settings className="w-4 h-4" />
          </Button>
          <Button
            variant="secondary"
            size="icon"
            onClick={() => setShowEditDialog(true)}
            className="bg-background/60 backdrop-blur-md border border-white/10"
            title="Редактировать"
          >
            <Pencil className="w-4 h-4" />
          </Button>
          <Button
            variant="secondary"
            size="icon"
            onClick={handleDelete}
            className="bg-background/60 backdrop-blur-md border border-white/10 hover:bg-destructive hover:text-destructive-foreground"
            title="Удалить"
          >
            <Trash2 className="w-4 h-4" />
          </Button>
        </div>
      </div>

      {/* Content */}
      <div className="relative z-10 px-6 pb-6 pt-6 sm:px-8">
        {/* Stats */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          <button
            type="button"
            className="group relative w-full rounded-2xl border border-border/60 bg-card/60 backdrop-blur-xl p-4 text-left transition-all hover:border-foreground/20 hover:shadow-[0_18px_45px_rgba(8,10,25,0.45)] min-h-[130px]"
            onClick={() => {
              setRatingDraft(userRating ?? 4);
              setShowRatingModal(true);
            }}
            aria-label={"Оценка"}
          >
            <div className="relative flex h-full flex-col justify-between gap-4">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <div className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    {"Оценка"}
                  </div>
                  <div className="mt-2 flex items-baseline gap-2">
                    <span className="text-3xl font-semibold text-foreground">
                      {displayRating ?? "?"}
                    </span>
                    <span className="text-xs text-muted-foreground">/7</span>
                  </div>
                </div>
                <div
                  className="h-11 w-11 rounded-full border border-white/10 bg-white/8 flex items-center justify-center text-white/90 shadow-[0_10px_24px_rgba(0,0,0,0.22)]"
                >
                  <Star className="w-4 h-4" />
                </div>
              </div>
              <div className="h-1.5 w-full rounded-full bg-muted/40">
                <div
                  className="h-full rounded-full bg-foreground/90 transition-all duration-300"
                  style={{
                    width: `${displayRating ? (displayRating / 7) * 100 : 0}%`,
                  }}
                />
              </div>
            </div>
          </button>

          <div className="rounded-2xl border border-border/60 bg-card/60 backdrop-blur-xl p-4 shadow-[0_12px_30px_rgba(8,12,24,0.35)]">
            <div className="text-sm text-muted-foreground mb-1">
              {"Время в игре"}
            </div>
            <div className="text-2xl font-bold flex items-center gap-2">
              <Timer className="w-5 h-5" />
              {formatPlaytime(game.total_playtime)}
            </div>
          </div>

          <div className="rounded-2xl border border-border/60 bg-card/60 backdrop-blur-xl p-4 shadow-[0_12px_30px_rgba(8,12,24,0.35)] space-y-3">
            <div className="flex items-center justify-between gap-3">
              <div className="text-sm text-muted-foreground">
                {"Статус прохождения"}
              </div>
              <div className={cn("text-xs rounded-full px-2 py-0.5 border", playStatusClasses[playStatus])}>
                {playStatusLabel[playStatus]}
              </div>
            </div>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="outline"
                  className="w-full justify-between"
                >
                  <span className="flex items-center gap-2">
                    {(() => {
                      const Icon = playStatusIcon[playStatus];
                      return <Icon className="w-4 h-4" />;
                    })()}
                    {playStatusLabel[playStatus]}
                  </span>
                  <Check className="w-3.5 h-3.5 opacity-70" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start" className="w-56">
                <DropdownMenuRadioGroup
                  value={playStatus}
                  onValueChange={(value) => {
                    handleSavePlayStatus(value as Game["play_status"]);
                  }}
                >
                  {Object.entries(playStatusLabel).map(([value, label]) => (
                    <DropdownMenuRadioItem key={value} value={value}>
                      <div className="flex items-center gap-2">
                        {(() => {
                          const Icon = playStatusIcon[value as Game["play_status"]];
                          return <Icon className="w-4 h-4 opacity-80" />;
                        })()}
                        <span>{label}</span>
                      </div>
                    </DropdownMenuRadioItem>
                  ))}
                </DropdownMenuRadioGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>

        {/* Info Grid */}
        <div className="grid gap-5">
          {/* Description */}
          <div className="space-y-5">
            <div className="bg-card/60 backdrop-blur-xl rounded-2xl p-6 border border-border/60 shadow-[0_18px_40px_rgba(8,12,24,0.35)]">
              <h3 className="font-semibold text-lg mb-4">Об игре</h3>
              <p className="text-muted-foreground whitespace-pre-wrap leading-relaxed">
                {game.description || "Описание отсутствует."}
              </p>
            </div>

            {/* Details Table */}
            <div className="bg-card/60 backdrop-blur-xl rounded-2xl p-6 border border-border/60 shadow-[0_18px_40px_rgba(8,12,24,0.35)]">
              <h3 className="font-semibold text-lg mb-4">Детали</h3>
              <div className="grid grid-cols-2 gap-4 text-sm">
                {game.released && (
                  <div>
                    <span className="text-muted-foreground block mb-1">
                      Дата выхода
                    </span>
                    <span>{new Date(game.released).toLocaleDateString()}</span>
                  </div>
                )}
                {game.developers && (
                  <div>
                    <span className="text-muted-foreground block mb-1">
                      Разработчик
                    </span>
                    <span>{game.developers}</span>
                  </div>
                )}
                {game.publishers && (
                  <div>
                    <span className="text-muted-foreground block mb-1">
                      Издатель
                    </span>
                    <span>{game.publishers}</span>
                  </div>
                )}
                {game.platforms && (
                  <div>
                    <span className="text-muted-foreground block mb-1">
                      Платформы
                    </span>
                    <span>{game.platforms}</span>
                  </div>
                )}
              </div>

              <div className="mt-6 pt-4 border-t border-border/60 flex items-center gap-2 text-xs text-muted-foreground">
                <FolderOpen className="w-3 h-3" />
                <span className="font-mono truncate">{game.exe_path}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      {showGameSettings && (
        <div className="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50 p-4">
          <div className="bg-card/90 backdrop-blur-xl rounded-2xl border border-border/60 w-full max-w-xl shadow-[0_30px_80px_rgba(8,12,24,0.55)] overflow-hidden">
            <div className="flex items-center justify-between p-5 border-b border-border/60">
              <div>
                <div className="text-xs uppercase tracking-wider text-muted-foreground">
                  {"Настройки игры"}
                </div>
                <div className="text-lg font-semibold">{game.name}</div>
              </div>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setShowGameSettings(false)}
              >
                <X className="w-4 h-4" />
              </Button>
            </div>

            <ScrollArea className="max-h-[70vh]">
              <div className="p-5 space-y-6">
                <div className="rounded-2xl border border-border/60 bg-secondary/30 p-4 space-y-3">
                  <div className="text-[10px] uppercase tracking-wider text-muted-foreground">
                    {"Бэкапы"}
                  </div>
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <div className="text-sm font-medium">{"Автобэкап"}</div>
                      <div className="text-xs text-muted-foreground">
                        {"Автоматическо создавать копию после выхода из игры"}
                      </div>
                    </div>
                    <Switch
                      checked={game.backup_enabled}
                      onCheckedChange={(checked) =>
                        toggleBackupEnabled(checked)
                      }
                    />
                  </div>
                </div>

                <div className="rounded-2xl border border-border/60 bg-secondary/30 p-4 space-y-3">
                  <div className="flex items-center justify-between">
                    <div className="text-[10px] uppercase tracking-wider text-muted-foreground">
                      {"Путь сохранений"}
                    </div>
                    <span
                      className={cn(
                        "text-[10px] uppercase tracking-wider",
                        game.save_path
                          ? "text-emerald-400"
                          : "text-muted-foreground",
                      )}
                    >
                      {game.save_path ? "Настроен" : "Не найдено"}
                    </span>
                  </div>

                  <Input
                    value={savePathDraft}
                    onChange={(event) => setSavePathDraft(event.target.value)}
                    placeholder={"Путь к сохранениям"}
                  />

                  <div className="flex items-center justify-between gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={handleInsertGamePathToken}
                      className="text-xs"
                    >
                      {"Вставить"}
                      <span className="ml-2 rounded-md border border-emerald-500/40 bg-emerald-500/10 px-1.5 py-0.5 font-mono text-[10px] text-emerald-200">
                        {gamePathToken}
                      </span>
                    </Button>
                    {savePathValue.includes(gamePathToken) && (
                      <span className="text-[11px] text-muted-foreground">
                        {"Можно использовать путь от папки игры"}
                      </span>
                    )}
                  </div>
                  {renderSavePathPreview()}

                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      size="icon"
                      onClick={handleSelectSavePath}
                      title="Выбрать папку"
                    >
                      <FolderOpen className="w-4 h-4" />
                    </Button>
                    <Button
                      variant="outline"
                      size="icon"
                      onClick={handleSelectSaveFile}
                      title="Выбрать файл"
                    >
                      <FileIcon className="w-4 h-4" />
                    </Button>
                    <Button
                      variant="outline"
                      size="icon"
                      onClick={handleOpenSavePath}
                      disabled={!savePathValue}
                      title="Открыть папку"
                    >
                      <ExternalLink className="w-4 h-4" />
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={handleLocateSavePath}
                      disabled={locatingSavePath}
                      className="text-xs"
                    >
                      {locatingSavePath ? (
                        <Loader2 className="w-3 h-3 animate-spin" />
                      ) : (
                        <Search className="w-3 h-3" />
                      )}
                      {"Найти"}
                    </Button>
                    <Button
                      size="sm"
                      onClick={handleSaveSavePath}
                      disabled={savingSavePath || !savePathDirty}
                      className="text-xs"
                    >
                      {savingSavePath ? (
                        <Loader2 className="w-3 h-3 animate-spin" />
                      ) : (
                        <Save className="w-3 h-3" />
                      )}
                      {"Сохранить"}
                    </Button>
                  </div>

                  <p className="text-xs text-muted-foreground">
                    {
                      "Можно указать папку или конкретный файл сохранения. Если путь пустой, SQOBA попробует найти сохранения при следующем бэкапе. Можно использовать {PATHTOGAME} как папку игры для относительных путей."
                    }
                  </p>
                </div>

                <div className="rounded-2xl border border-border/60 bg-secondary/30 p-4 space-y-3">
                  <div className="text-[10px] uppercase tracking-wider text-muted-foreground">
                    {"История бэкапов"}
                  </div>
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <div className="text-sm font-medium">
                        {"Сохранения игры"}
                      </div>
                      <div className="text-xs text-muted-foreground">
                        {"Создайте вручную или используйте автобэкап."}
                      </div>
                    </div>
                    <Button
                      variant="outline"
                      size="icon"
                      className="h-8 w-8"
                      onClick={createManualBackup}
                      title="Создать бэкап"
                    >
                      <Download className="w-4 h-4" />
                    </Button>
                  </div>

                  {loadingBackups ? (
                    <div className="flex items-center justify-center py-8">
                      <Loader2 className="w-6 h-6 animate-spin" />
                    </div>
                  ) : backups.length === 0 ? (
                    <div className="text-center py-4 text-muted-foreground">
                      <HardDrive className="w-8 h-8 mx-auto mb-2 opacity-50" />
                      <p className="text-sm">Нет бэкапов</p>
                    </div>
                  ) : (
                    <div className="space-y-2">
                      {latestBackup && (
                        <div className="rounded-xl border border-border/60 bg-secondary/50 p-3">
                          <div className="flex items-start justify-between gap-3">
                            <div className="min-w-0">
                              <div className="text-[10px] uppercase tracking-wider text-muted-foreground mb-1">
                                Актуальный бэкап
                              </div>
                              <div className="text-sm font-medium">
                                {new Date(
                                  latestBackup.created_at,
                                ).toLocaleDateString()}
                                <span className="text-muted-foreground ml-1">
                                  {new Date(
                                    latestBackup.created_at,
                                  ).toLocaleTimeString([], {
                                    hour: "2-digit",
                                    minute: "2-digit",
                                  })}
                                </span>
                              </div>
                              <div className="text-xs text-muted-foreground flex items-center gap-2">
                                <span>
                                  {(
                                    latestBackup.backup_size /
                                    1024 /
                                    1024
                                  ).toFixed(1)}{" "}
                                  МБ
                                </span>
                                {latestBackup.is_auto && (
                                  <span className="bg-primary/10 text-primary px-1 rounded">
                                    Авто
                                  </span>
                                )}
                              </div>
                            </div>
                            <Button
                              size="icon"
                              variant="ghost"
                              className="h-8 w-8"
                              onClick={() => restoreBackup(latestBackup.id)}
                              title="Восстановить"
                            >
                              <Upload className="w-4 h-4" />
                            </Button>
                          </div>
                        </div>
                      )}

                      {olderBackups.length > 0 && (
                        <div>
                          <Button
                            variant="ghost"
                            size="sm"
                            className="h-7 px-2 text-xs text-muted-foreground"
                            onClick={() => setShowAllBackups(!showAllBackups)}
                          >
                            {showAllBackups
                              ? "Скрыть историю"
                              : `Показать историю (${olderBackups.length})`}
                          </Button>
                          {showAllBackups && (
                            <ScrollArea className="h-[240px] pr-3 mt-2">
                              <div className="space-y-2">
                                {olderBackups.map((backup) => (
                                  <div
                                    key={backup.id}
                                    className="flex items-center justify-between p-3 rounded-xl bg-secondary/50 border border-transparent hover:border-border/60 transition-colors group"
                                  >
                                    <div className="min-w-0">
                                      <div className="text-sm font-medium">
                                        {new Date(
                                          backup.created_at,
                                        ).toLocaleDateString()}
                                        <span className="text-muted-foreground ml-1">
                                          {new Date(
                                            backup.created_at,
                                          ).toLocaleTimeString([], {
                                            hour: "2-digit",
                                            minute: "2-digit",
                                          })}
                                        </span>
                                      </div>
                                      <div className="text-xs text-muted-foreground flex items-center gap-2">
                                        <span>
                                          {(backup.backup_size / 1024 / 1024).toFixed(
                                            1,
                                          )}{" "}
                                          МБ
                                        </span>
                                        {backup.is_auto && (
                                          <span className="bg-primary/10 text-primary px-1 rounded">
                                            Авто
                                          </span>
                                        )}
                                      </div>
                                    </div>
                                    <Button
                                      size="icon"
                                      variant="ghost"
                                      className="h-8 w-8 opacity-0 group-hover:opacity-100 transition-opacity"
                                      onClick={() => restoreBackup(backup.id)}
                                      title="Восстановить"
                                    >
                                      <Upload className="w-4 h-4" />
                                    </Button>
                                  </div>
                                ))}
                              </div>
                            </ScrollArea>
                          )}
                        </div>
                      )}
                    </div>
                  )}
                </div>

                <div className="rounded-2xl border border-border/60 bg-secondary/30 p-4 space-y-3">
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <div className="text-[10px] uppercase tracking-wider text-muted-foreground">
                        {"Заметка"}
                      </div>
                      <div className="text-sm font-medium">
                        {"Личные заметки по игре"}
                      </div>
                    </div>
                    <Button
                      size="sm"
                      onClick={handleSaveUserNote}
                      disabled={savingUserNote}
                      className="text-xs"
                    >
                      {savingUserNote ? (
                        <Loader2 className="w-3 h-3 animate-spin" />
                      ) : (
                        <Save className="w-3 h-3" />
                      )}
                      {"Сохранить"}
                    </Button>
                  </div>
                  <textarea
                    value={userNote}
                    onChange={(event) => setUserNote(event.target.value)}
                    placeholder={"Что важно помнить перед следующим запуском?"}
                    className="min-h-[120px] w-full rounded-xl border border-border/60 bg-background/50 px-3 py-2 text-sm outline-none transition-colors focus:border-primary/50 focus:ring-2 focus:ring-primary/15"
                  />
                  <p className="text-xs text-muted-foreground">
                    {
                      "Заметка сохраняется отдельно от рейтинга и помогает держать контекст по прохождению, модам и сохранениям."
                    }
                  </p>
                </div>
              </div>
            </ScrollArea>

            <div className="p-5 border-t border-border/60 flex justify-end">
              <Button
                variant="ghost"
                onClick={() => setShowGameSettings(false)}
              >
                {"Закрыть"}
              </Button>
            </div>
          </div>
        </div>
      )}

      {showRatingModal && (
        <div className="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50 p-4">
          <div
            className="bg-card/90 backdrop-blur-xl rounded-2xl border border-border/60 w-full max-w-sm p-6 shadow-[0_30px_80px_rgba(8,12,24,0.55)]"
          >
            <div className="flex items-center justify-between mb-6">
              <div
                className="w-12 h-12 rounded-full border border-white/10 bg-white/8 shadow-[0_10px_24px_rgba(0,0,0,0.22)] flex items-center justify-center text-2xl font-bold text-white"
              >
                {ratingDraft}
              </div>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setShowRatingModal(false)}
              >
                <X className="w-4 h-4" />
              </Button>
            </div>

            <input
              type="number"
              min={1}
              max={7}
              step={1}
              value={ratingDraft}
              onChange={(e) => {
                const value = Math.max(
                  1,
                  Math.min(7, parseInt(e.target.value || "1", 10)),
                );
                setRatingDraft(value);
              }}
              className="w-full text-center text-6xl font-bold bg-background/20 border border-border/60 rounded-2xl py-6 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
            />

            <div className="flex justify-end gap-2 mt-6">
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setShowRatingModal(false)}
              >
                <X className="w-4 h-4" />
              </Button>
              <Button
                size="icon"
                disabled={savingUserRating}
                onClick={() => {
                  setShowRatingModal(false);
                  handleSaveUserRating(ratingDraft);
                }}
              >
                {savingUserRating ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <Save className="w-4 h-4" />
                )}
              </Button>
            </div>
          </div>
        </div>
      )}

      {showDescriptionModal && heroDescription && (
        <div
          className="fixed inset-y-0 left-1/2 z-[120] flex w-full max-w-[1520px] -translate-x-1/2 items-center justify-center bg-black/78 backdrop-blur-sm p-4"
          onMouseDown={(event: ReactMouseEvent<HTMLDivElement>) => {
            if (event.target === event.currentTarget) {
              setShowDescriptionModal(false);
            }
          }}
        >
          <div
            data-testid="game-detail-description-modal"
            className="w-full max-w-2xl overflow-hidden rounded-2xl border border-border/60 bg-card/92 shadow-[0_30px_80px_rgba(8,12,24,0.55)] backdrop-blur-xl"
          >
            <div className="flex items-center justify-between border-b border-border/60 px-5 py-4">
              <h2 className="text-lg font-semibold text-foreground">Описание игры</h2>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setShowDescriptionModal(false)}
                aria-label="Закрыть описание"
              >
                <X className="w-4 h-4" />
              </Button>
            </div>
            <ScrollArea className="max-h-[70vh]">
              <div className="px-5 py-4">
                <p className="whitespace-pre-wrap text-sm leading-7 text-foreground/88 sm:text-[15px]">
                  {heroDescription}
                </p>
              </div>
            </ScrollArea>
          </div>
        </div>
      )}

      {backupProgress.active && (
        <div className="fixed bottom-4 left-4 right-4 lg:left-[280px] z-50">
          <div className="bg-card/80 backdrop-blur-xl border border-border/60 rounded-xl px-4 py-3 shadow-[0_16px_40px_rgba(8,12,24,0.45)] flex items-center gap-3">
            <Loader2 className="w-4 h-4 animate-spin text-primary" />
            <div className="min-w-0">
              <div className="text-sm font-medium">
                {backupProgress.stage === "scan"
                  ? "Подготовка бэкапа"
                  : backupProgress.stage === "copy"
                    ? "Создание бэкапа"
                    : backupProgress.stage === "restore"
                      ? "Восстановление бэкапа"
                      : "Обработка"}
              </div>
              <div className="text-xs text-muted-foreground truncate">
                {backupProgress.message}
              </div>
            </div>
            {backupProgress.total > 0 && (
              <div className="ml-auto text-xs text-muted-foreground tabular-nums">
                {backupProgress.done}/{backupProgress.total}
              </div>
            )}
          </div>
        </div>
      )}

      {/* EDIT GAME DIALOG */}
      {showEditDialog && (
        <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-50 p-4 animate-in fade-in duration-200">
          <div className="bg-card rounded-xl border w-full max-w-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
            <div className="flex items-center justify-between p-6 border-b">
              <h2 className="text-xl font-bold">Редактировать игру</h2>
              <div className="flex items-center gap-2">
                <Button onClick={handleSaveEdit} disabled={saving}>
                  {saving ? (
                    <Loader2 className="w-4 h-4 animate-spin mr-2" />
                  ) : (
                    <Save className="w-4 h-4 mr-2" />
                  )}
                  {"Сохранить"}
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => setShowEditDialog(false)}
                >
                  <X className="w-5 h-5" />
                </Button>
              </div>
            </div>
            <div className="flex-1 min-h-0 overflow-y-auto">
              <div className="p-6 space-y-6">
                <div className="space-y-2">
                  <label className="text-sm font-medium" htmlFor="edit-name">
                    Название
                  </label>
                  <Input
                    id="edit-name"
                    value={editForm.name}
                    onChange={(e) =>
                      setEditForm((prev) => ({ ...prev, name: e.target.value }))
                    }
                  />
                </div>

                <div className="space-y-2">
                  <label
                    className="text-sm font-medium"
                    htmlFor="edit-description"
                  >
                    Описание
                  </label>
                  <textarea
                    id="edit-description"
                    className="flex min-h-[120px] w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
                    value={editForm.description}
                    onChange={(e) =>
                      setEditForm((prev) => ({
                        ...prev,
                        description: e.target.value,
                      }))
                    }
                  />
                </div>

                <div className="space-y-5">
                  <div className="space-y-2">
                    <label
                      className="text-sm font-medium block"
                      htmlFor="edit-background-url"
                    >
                      URL фона
                    </label>

                    <div className="flex gap-2">
                      <Input
                        id="edit-background-url"
                        value={editForm.background_image}
                        onChange={(e) =>
                          setEditForm((prev) => ({
                            ...prev,
                            background_image: e.target.value,
                          }))
                        }
                        placeholder="https://..."
                        className="flex-1"
                      />
                      <Button
                        variant="outline"
                        onClick={() =>
                          handleSearchGoogleImage(editForm.name || game?.name || "", "background")
                        }
                        title="Найти в Google"
                      >
                        <Globe className="w-4 h-4 mr-2" />
                        Google
                      </Button>
                    </div>

                    <div className="aspect-video w-full rounded-lg border bg-muted/50 overflow-hidden relative">
                      {editForm.background_image ? (
                        <img
                          src={editForm.background_image}
                           alt={editForm.name ? `${editForm.name} background` : "Background"}
                          className="w-full h-full object-cover"
                          onError={(e) => {
                            (e.target as HTMLImageElement).src = "";
                          }}
                        />
                      ) : (
                        <div className="flex items-center justify-center h-full text-muted-foreground">
                          <ImageIcon className="w-8 h-8 mr-2 opacity-50" />
                           Нет изображения
                        </div>
                      )}
                    </div>
                  </div>

                  <div className="space-y-2">
                    <label
                      className="text-sm font-medium block"
                      htmlFor="edit-cover-url"
                    >
                      URL обложки
                    </label>

                    <div className="flex gap-2">
                      <Input
                        id="edit-cover-url"
                        value={editForm.cover_image}
                        onChange={(e) =>
                          setEditForm((prev) => ({
                            ...prev,
                            cover_image: e.target.value,
                          }))
                        }
                        placeholder="https://..."
                        className="flex-1"
                      />
                      <Button
                        variant="outline"
                        onClick={() =>
                          handleSearchGoogleImage(editForm.name || game?.name || "", "cover")
                        }
                        title="Найти в Google"
                      >
                        <Globe className="w-4 h-4 mr-2" />
                        Google
                      </Button>
                    </div>

                    <div className="aspect-[2/3] w-40 rounded-lg border bg-muted/50 overflow-hidden relative">
                      {editForm.cover_image ? (
                        <img
                          src={editForm.cover_image}
                           alt={editForm.name ? `${editForm.name} cover` : "Cover"}
                          className="w-full h-full object-cover"
                          onError={(e) => {
                            (e.target as HTMLImageElement).src = "";
                          }}
                        />
                      ) : (
                        <div className="flex items-center justify-center h-full text-muted-foreground text-xs text-center px-2">
                          Нет обложки
                        </div>
                      )}
                    </div>
                  </div>

                </div>
              </div>
            </div>
            <div className="p-6 border-t bg-muted/20 flex justify-end gap-2">
              <Button variant="ghost" onClick={() => setShowEditDialog(false)}>
                Отмена
              </Button>
              <Button onClick={handleSaveEdit} disabled={saving}>
                {saving ? (
                  <Loader2 className="w-4 h-4 animate-spin mr-2" />
                ) : (
                  <Save className="w-4 h-4 mr-2" />
                )}
                Сохранить
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* RAWG Search Modal (Existing) */}
      {showMetadataSearch && (
        <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-50 p-4 animate-in fade-in duration-200">
          <div className="bg-card rounded-lg w-full max-w-lg max-h-[80vh] flex flex-col">
            <div className="p-4 border-b">
              <h2 className="text-lg font-semibold">Поиск метаданных</h2>
              <p className="text-sm text-muted-foreground">
                Поиск информации об игре в базе RAWG
              </p>
            </div>

            <div className="p-4 flex-1 overflow-hidden flex flex-col">
              <div className="flex gap-2 mb-4">
                <Input
                  placeholder="Название игры..."
                  value={metadataQuery}
                  onChange={(e) => setMetadataQuery(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && searchMetadata()}
                  autoFocus
                />
                <Button onClick={searchMetadata} disabled={searchingMetadata}>
                  {searchingMetadata ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <Search className="w-4 h-4" />
                  )}
                </Button>
              </div>
              <div className="flex items-center justify-between gap-3 text-sm text-muted-foreground mb-3">
                <span id="rawg-rename-toggle">
                  Использовать название из RAWG
                </span>
                <Switch
                  checked={renameFromMetadata}
                  onCheckedChange={setRenameFromMetadata}
                  aria-labelledby="rawg-rename-toggle"
                />
              </div>
              <ScrollArea className="flex-1">
                {metadataResults.length > 0 ? (
                  <div className="space-y-2">
                    {metadataResults.map((result) => (
                      <button
                        type="button"
                        key={result.id}
                        className="w-full text-left flex items-center gap-3 p-3 rounded-md hover:bg-secondary cursor-pointer border border-transparent hover:border-border transition-colors"
                        onClick={() => applyMetadata(result)}
                      >
                        {result.background_image ? (
                          <img
                            src={result.background_image}
                            alt={result.name}
                            className="w-16 h-16 object-cover rounded"
                          />
                        ) : (
                          <div className="w-16 h-16 bg-muted rounded flex items-center justify-center">
                            <Gamepad2 className="w-6 h-6 text-muted-foreground" />
                          </div>
                        )}
                        <div className="flex-1 min-w-0">
                          <div className="font-medium truncate">
                            {result.name}
                          </div>
                          <div className="text-sm text-muted-foreground">
                            {result.released?.slice(0, 4)}
                            {result.metacritic && ` • ${result.metacritic}`}
                          </div>
                        </div>
                        {applyingMetadata ? (
                          <Loader2 className="w-4 h-4 animate-spin" />
                        ) : (
                          <ExternalLink className="w-4 h-4 text-muted-foreground" />
                        )}
                      </button>
                    ))}
                  </div>
                ) : (
                  <div className="text-center py-8 text-muted-foreground">
                    Введите название для поиска
                  </div>
                )}
              </ScrollArea>
            </div>

            <div className="p-4 border-t flex justify-end">
              <Button
                variant="ghost"
                onClick={() => setShowMetadataSearch(false)}
              >
                Закрыть
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* Backup Prompt Modal */}
      {showBackupPrompt && (
        <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4">
          <div className="bg-card rounded-lg w-full max-w-md p-6">
            <h2 className="text-lg font-semibold mb-2">Создать бэкап?</h2>
            <p className="text-muted-foreground mb-4">
              Ваши сохранения изменились с момента последнего бэкапа. Создать
              копию перед запуском?
            </p>
            <div className="flex justify-end gap-2">
              <Button
                variant="ghost"
                onClick={() => {
                  setShowBackupPrompt(false);
                  launchGame();
                }}
              >
                Пропустить
              </Button>
              <Button onClick={handleBackupAndLaunch}>Бэкап и Запуск</Button>
            </div>
          </div>
        </div>
      )}
      {showRestorePrompt && (
        <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4">
          <div className="bg-card rounded-lg w-full max-w-md p-6">
            <h2 className="text-lg font-semibold mb-2">Восстановить бэкап?</h2>
            <p className="text-muted-foreground mb-4">
              Текущий размер сохранений меньше, чем в последнем бэкапе.
              Восстановить бэкап перед запуском?
            </p>
            {restoreInfo && (
              <div className="text-xs text-muted-foreground mb-4">
                Текущий: {(restoreInfo.current_size / 1024 / 1024).toFixed(1)}{" "}
                МБ Бэкап: {(restoreInfo.backup_size / 1024 / 1024).toFixed(1)}{" "}
                МБ
              </div>
            )}
            <div className="flex justify-end gap-2">
              <Button
                variant="ghost"
                onClick={handleSkipRestore}
                disabled={restoring}
              >
                Пропустить
              </Button>
              <Button onClick={handleRestoreAndLaunch} disabled={restoring}>
                {restoring ? (
                  <Loader2 className="w-4 h-4 animate-spin mr-2" />
                ) : null}
                Восстановить и запустить
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}





