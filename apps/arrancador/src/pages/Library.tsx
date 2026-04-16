import {
  Check,
  Clock,
  Gamepad2,
  Grid3X3,
  ListFilter,
  List,
  SortAsc,
  Play,
  Search,
  Star,
  Filter,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { GameCard } from "@/components/GameCard";
import { RawgMetadataPrompt } from "@/components/RawgMetadataPrompt";
import { useToast } from "@/components/ToastProvider";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { gamesApi } from "@/lib/api";
import { useDropZone } from "@/hooks/useDropZone";
import { cn } from "@/lib/utils";
import { useGamesActions, useGamesState } from "@/store/GamesContext";
import type { Game, NewGame } from "@/types";

type ViewMode = "grid" | "list";
type SortBy = "name" | "lastPlayed" | "dateAdded" | "playCount" | "playtime";
type PlayedState = "all" | "played" | "unplayed";
type RatingMode = "user" | "metacritic";
type PlayStatusState =
  | "all"
  | "not_started"
  | "in_progress"
  | "completed"
  | "abandoned";

const sortByLabel: Record<SortBy, string> = {
  name: "По имени",
  lastPlayed: "Недавно запущенные",
  dateAdded: "Недавно добавленные",
  playCount: "По запускам",
  playtime: "По времени",
};

const playedStateLabel: Record<PlayedState, string> = {
  all: "Все",
  played: "Играли",
  unplayed: "Не играли",
};
type InstallState = "all" | "installed" | "not_installed";
const installStateLabel: Record<InstallState, string> = {
  all: "Все",
  installed: "Установленные",
  not_installed: "Не установленные",
};
const playStatusLabel: Record<PlayStatusState, string> = {
  all: "Все",
  not_started: "Не начато",
  in_progress: "В процессе",
  completed: "Пройдено",
  abandoned: "Брошено",
};

const FILTER_PRESET_STORAGE_KEY = "arrancador_library_filter_preset_v1";

type LibraryFilterPreset = {
  searchQuery: string;
  viewMode: ViewMode;
  sortBy: SortBy;
  showFavoritesOnly: boolean;
  showAdvancedFilters: boolean;
  selectedGenres: string[];
  selectedPlatforms: string[];
  playedState: PlayedState;
  installState: InstallState;
  playStatusState: PlayStatusState;
  ratingMode: RatingMode;
  minRating: string;
  maxRating: string;
  minMetacritic: string;
  maxMetacritic: string;
  minPlaytimeHours: string;
  maxPlaytimeHours: string;
  requireMetadata: boolean;
};

const defaultLibraryFilterPreset: LibraryFilterPreset = {
  searchQuery: "",
  viewMode: "grid",
  sortBy: "name",
  showFavoritesOnly: false,
  showAdvancedFilters: false,
  selectedGenres: [],
  selectedPlatforms: [],
  playedState: "all",
  installState: "all",
  playStatusState: "all",
  ratingMode: "user",
  minRating: "",
  maxRating: "",
  minMetacritic: "",
  maxMetacritic: "",
  minPlaytimeHours: "",
  maxPlaytimeHours: "",
  requireMetadata: false,
};

const createDefaultFilterPreset = (): LibraryFilterPreset => ({
  ...defaultLibraryFilterPreset,
  selectedGenres: [],
  selectedPlatforms: [],
});

const isViewMode = (value: unknown): value is ViewMode =>
  value === "grid" || value === "list";
const isSortBy = (value: unknown): value is SortBy =>
  value === "name" ||
  value === "lastPlayed" ||
  value === "dateAdded" ||
  value === "playCount" ||
  value === "playtime";
const isPlayedState = (value: unknown): value is PlayedState =>
  value === "all" || value === "played" || value === "unplayed";
const isInstallState = (value: unknown): value is InstallState =>
  value === "all" || value === "installed" || value === "not_installed";
const isPlayStatusState = (value: unknown): value is PlayStatusState =>
  value === "all" ||
  value === "not_started" ||
  value === "in_progress" ||
  value === "completed" ||
  value === "abandoned";
const isRatingMode = (value: unknown): value is RatingMode =>
  value === "user" || value === "metacritic";

const sanitizeStringArray = (value: unknown): string[] => {
  if (!Array.isArray(value)) return [];
  return value.filter((item) => typeof item === "string");
};

const loadLibraryFilterPreset = (): LibraryFilterPreset => {
  const fallback = createDefaultFilterPreset();
  if (typeof window === "undefined") return fallback;
  const stored = window.localStorage.getItem(FILTER_PRESET_STORAGE_KEY);
  if (!stored) return fallback;

  try {
    const raw = JSON.parse(stored);
    if (!raw || typeof raw !== "object") return fallback;

    return {
      ...fallback,
      searchQuery:
        typeof raw.searchQuery === "string"
          ? raw.searchQuery
          : fallback.searchQuery,
      viewMode: isViewMode(raw.viewMode)
        ? raw.viewMode
        : fallback.viewMode,
      sortBy: isSortBy(raw.sortBy)
        ? raw.sortBy
        : fallback.sortBy,
      showFavoritesOnly:
        typeof raw.showFavoritesOnly === "boolean"
          ? raw.showFavoritesOnly
          : fallback.showFavoritesOnly,
      showAdvancedFilters:
        typeof raw.showAdvancedFilters === "boolean"
          ? raw.showAdvancedFilters
          : fallback.showAdvancedFilters,
      selectedGenres: sanitizeStringArray(raw.selectedGenres),
      selectedPlatforms: sanitizeStringArray(raw.selectedPlatforms),
      playedState: isPlayedState(raw.playedState)
        ? raw.playedState
        : fallback.playedState,
      installState: isInstallState(raw.installState)
        ? raw.installState
        : fallback.installState,
      playStatusState: isPlayStatusState(raw.playStatusState)
        ? raw.playStatusState
        : fallback.playStatusState,
      ratingMode: isRatingMode(raw.ratingMode)
        ? raw.ratingMode
        : fallback.ratingMode,
      minRating:
        typeof raw.minRating === "string"
          ? raw.minRating
          : fallback.minRating,
      maxRating:
        typeof raw.maxRating === "string"
          ? raw.maxRating
          : fallback.maxRating,
      minMetacritic:
        typeof raw.minMetacritic === "string"
          ? raw.minMetacritic
          : fallback.minMetacritic,
      maxMetacritic:
        typeof raw.maxMetacritic === "string"
          ? raw.maxMetacritic
          : fallback.maxMetacritic,
      minPlaytimeHours:
        typeof raw.minPlaytimeHours === "string"
          ? raw.minPlaytimeHours
          : fallback.minPlaytimeHours,
      maxPlaytimeHours:
        typeof raw.maxPlaytimeHours === "string"
          ? raw.maxPlaytimeHours
          : fallback.maxPlaytimeHours,
      requireMetadata:
        typeof raw.requireMetadata === "boolean"
          ? raw.requireMetadata
          : fallback.requireMetadata,
    };
  } catch {
    return fallback;
  }
};

const persistLibraryFilterPreset = (preset: LibraryFilterPreset) => {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(
      FILTER_PRESET_STORAGE_KEY,
      JSON.stringify(preset),
    );
  } catch {
    // no-op
  }
};

const parseNumber = (value: string) => {
  if (!value.trim()) return null;
  const parsed = Number.parseFloat(value.replace(",", "."));
  return Number.isFinite(parsed) ? parsed : null;
};

const splitList = (value: string | null) =>
  value
    ?.split(",")
    .map((value) => value.trim())
    .filter(Boolean) ?? [];

// Helper to format playtime
function formatPlaytime(seconds: number) {
  if (!seconds) return "0 ч";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) return `${hours} ч ${minutes} мин`;
  return `${minutes} мин`;
}

function fileNameFromPath(filePath: string) {
  const normalized = filePath.replace(/\\/g, "/");
  const name = normalized.split("/").pop();
  return name || filePath;
}

function cleanNameFromFile(fileName: string) {
  const base = fileName.replace(/\.exe$/i, "");
  return base.replace(/[-_]/g, " ").replace(/\s+/g, " ").trim() || base;
}

const normalizeNameForMerge = (value: string) => {
  return value
    .normalize("NFKD")
    .toLowerCase()
    .replace(/\.exe$/i, "")
    .replace(/\[[^\]]*\]|\([^\)]*\)|\{[^\}]*\}/g, " ")
    .replace(/[-_./]/g, " ")
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
};

const matchMergeNames = (candidate: string, incoming: string) => {
  if (!candidate || !incoming) return false;
  if (candidate === incoming) return true;
  if (
    candidate.length > 4 &&
    incoming.length > 4 &&
    (candidate.includes(incoming) || incoming.includes(candidate))
  ) {
    return true;
  }

  const candidateTokens = candidate.split(" ").filter(Boolean);
  const incomingTokens = incoming.split(" ").filter(Boolean);
  if (candidateTokens.length <= 1 || incomingTokens.length <= 1) return false;

  const tokenSet = new Set(incomingTokens);
  const overlap = candidateTokens.filter((token) => tokenSet.has(token)).length;
  if (overlap < 2) return false;

  return overlap / Math.max(candidateTokens.length, incomingTokens.length) >= 0.67;
};

const findMergeCandidate = (games: Game[], name: string): Game | undefined => {
  const normalized = normalizeNameForMerge(name);
  if (!normalized) return undefined;

  for (const game of games) {
    const candidateNames = [game.name, game.exe_name, fileNameFromPath(game.exe_path)];
    const exact = candidateNames.find(
      (candidateName) =>
        normalizeNameForMerge(candidateName) === normalized,
    );
    if (exact) return game;
  }

  return games.find((game) =>
    [game.name, game.exe_name, fileNameFromPath(game.exe_path)].some(
      (candidateName) => {
        const normalizedCandidate = normalizeNameForMerge(candidateName);
        return matchMergeNames(normalizedCandidate, normalized);
      },
    ),
  );
};

export default function Library() {
  const { games, loading, favorites } = useGamesState();
  const { addGames, refreshGames, updateGame } = useGamesActions();
  const { notify } = useToast();
  const initialFilters = useMemo(() => loadLibraryFilterPreset(), []);
  const [searchQuery, setSearchQuery] = useState(initialFilters.searchQuery);
  const [viewMode, setViewMode] = useState<ViewMode>(initialFilters.viewMode);
  const [sortBy, setSortBy] = useState<SortBy>(initialFilters.sortBy);
  const [showFavoritesOnly, setShowFavoritesOnly] = useState(
    initialFilters.showFavoritesOnly,
  );
  const [showAdvancedFilters, setShowAdvancedFilters] = useState(
    initialFilters.showAdvancedFilters,
  );
  const [selectedGenres, setSelectedGenres] = useState<string[]>(
    initialFilters.selectedGenres,
  );
  const [selectedPlatforms, setSelectedPlatforms] = useState<string[]>(
    initialFilters.selectedPlatforms,
  );
  const [playedState, setPlayedState] = useState<PlayedState>(
    initialFilters.playedState,
  );
  const [installState, setInstallState] = useState<InstallState>(
    initialFilters.installState,
  );
  const [playStatusState, setPlayStatusState] = useState<PlayStatusState>(
    initialFilters.playStatusState,
  );
  const [ratingMode, setRatingMode] = useState<RatingMode>(
    initialFilters.ratingMode,
  );
  const [minRating, setMinRating] = useState(initialFilters.minRating);
  const [maxRating, setMaxRating] = useState(initialFilters.maxRating);
  const [minMetacritic, setMinMetacritic] = useState(
    initialFilters.minMetacritic,
  );
  const [maxMetacritic, setMaxMetacritic] = useState(
    initialFilters.maxMetacritic,
  );
  const [minPlaytimeHours, setMinPlaytimeHours] = useState(
    initialFilters.minPlaytimeHours,
  );
  const [maxPlaytimeHours, setMaxPlaytimeHours] = useState(
    initialFilters.maxPlaytimeHours,
  );
  const [requireMetadata, setRequireMetadata] = useState(
    initialFilters.requireMetadata,
  );

  const [metadataQueue, setMetadataQueue] = useState<Game[]>([]);
  const [installedById, setInstalledById] = useState<Record<string, boolean>>({});

  const enqueueMetadata = useCallback((added: Game[]) => {
    setMetadataQueue((prev) => {
      if (added.length === 0) return prev;
      const seen = new Set(prev.map((g) => g.id));
      const next = [...prev];
      for (const g of added) {
        if (seen.has(g.id)) continue;
        seen.add(g.id);
        next.push(g);
      }
      return next;
    });
  }, []);

  const handleDroppedPaths = useCallback(
    async (paths: string[]) => {
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
            resolved = await gamesApi.resolveShortcutTarget(rawPath);
          } catch (e) {
            console.error("Failed to resolve shortcut:", e);
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

        const exists = await gamesApi.existsByPath(resolved).catch(() => false);
        if (exists) {
          skipped += 1;
          continue;
        }

        const exeName = fileNameFromPath(resolved);
        const name = cleanNameFromFile(exeName);
        const candidate = findMergeCandidate(games, name);
        if (candidate) {
          const doMerge = window.confirm(
            `Найдена игра с похожим названием: "${candidate.name}" из библиотеки. Обновить путь для существующей записи?`,
          );
          if (doMerge) {
            await updateGame(candidate.id, {
              exe_path: resolved,
            });
            merged += 1;
            continue;
          }
        }
        toAdd.push({ name, exe_path: resolved, exe_name: exeName });
      }

      if (toAdd.length > 0) {
        try {
          const added = await addGames(toAdd);
          enqueueMetadata(added);

          const extra: string[] = [];
          if (skipped > 0) {
            extra.push(`Пропущено: ${skipped}`);
          }
          if (invalid > 0) {
            extra.push(`Не поддерживается: ${invalid}`);
          }

          if (merged > 0) {
            extra.push(`Объединено с существующими: ${merged}`);
          }

          notify({
            tone: "success",
            title: `Добавлено ${added.length}`,
            description: extra.length ? extra.join(" | ") : undefined,
          });
        } catch (e) {
          console.error("Failed to add dropped games:", e);
          notify({
            tone: "error",
            title: "Не удалось добавить игру",
            description: "Проверьте путь к файлу.",
          });
        }
      } else {
        if (merged > 0) {
          notify({
            tone: "success",
            title: `Объединено с существующими: ${merged}`,
            description: "Обновлены пути для игр с совпавшим названием.",
          });
          return;
        }
        notify({
          tone: "warning",
          title: "Файлы не добавлены",
          description: "Поддерживаются .exe и .lnk.",
        });
      }
    },
    [addGames, enqueueMetadata, notify, games, updateGame],
  );

  const genreOptions = useMemo(() => {
    const values = new Set<string>();
    for (const game of games) {
      for (const genre of splitList(game.genres)) {
        values.add(genre);
      }
    }
    return Array.from(values).sort((a, b) => a.localeCompare(b));
  }, [games]);

  const platformOptions = useMemo(() => {
    const values = new Set<string>();
    for (const game of games) {
      for (const platform of splitList(game.platforms)) {
        values.add(platform);
      }
    }
    return Array.from(values).sort((a, b) => a.localeCompare(b));
  }, [games]);

  const clearAdvancedFilters = () => {
    setSelectedGenres([]);
    setSelectedPlatforms([]);
    setPlayedState("all");
    setInstallState("all");
    setPlayStatusState("all");
    setRatingMode("user");
    setMinRating("");
    setMaxRating("");
    setMinMetacritic("");
    setMaxMetacritic("");
    setMinPlaytimeHours("");
    setMaxPlaytimeHours("");
    setRequireMetadata(false);
    setShowFavoritesOnly(false);
    setSortBy("name");
  };

  useEffect(() => {
    let cancelled = false;

    const checkInstalled = async () => {
      if (games.length === 0) {
        setInstalledById({});
        return;
      }

      const resolved = await Promise.all(
        games.map(async (game) => {
          try {
            const value = await gamesApi.isInstalled(game.id);
            return [game.id, value] as const;
          } catch (error) {
            console.error("Failed to check install status:", error);
            return [game.id, true] as const;
          }
        }),
      );

      if (cancelled) return;

      const next: Record<string, boolean> = {};
      for (const [gameId, value] of resolved) {
        next[gameId] = value;
      }
      setInstalledById(next);
    };

    void checkInstalled();

    return () => {
      cancelled = true;
    };
  }, [games]);

  const hasActiveAdvancedFilters = useMemo(() => {
    const minRatingValue = parseNumber(minRating);
    const maxRatingValue = parseNumber(maxRating);
    const minMetacriticValue = parseNumber(minMetacritic);
    const maxMetacriticValue = parseNumber(maxMetacritic);
    const minPlaytimeValue = parseNumber(minPlaytimeHours);
    const maxPlaytimeValue = parseNumber(maxPlaytimeHours);

    return (
      showFavoritesOnly ||
      selectedGenres.length > 0 ||
      selectedPlatforms.length > 0 ||
      playedState !== "all" ||
      installState !== "all" ||
      playStatusState !== "all" ||
      requireMetadata ||
      minRatingValue !== null ||
      maxRatingValue !== null ||
      minMetacriticValue !== null ||
      maxMetacriticValue !== null ||
      minPlaytimeValue !== null ||
      maxPlaytimeValue !== null
    );
  }, [
    maxMetacritic,
    maxPlaytimeHours,
    maxRating,
    minMetacritic,
    minPlaytimeHours,
    minRating,
    playedState,
    installState,
    playStatusState,
    requireMetadata,
    selectedGenres.length,
    selectedPlatforms.length,
    showFavoritesOnly,
  ]);

  const activeFilterCount = useMemo(() => {
    const minRatingValue = parseNumber(minRating);
    const maxRatingValue = parseNumber(maxRating);
    const minMetacriticValue = parseNumber(minMetacritic);
    const maxMetacriticValue = parseNumber(maxMetacritic);
    const minPlaytimeValue = parseNumber(minPlaytimeHours);
    const maxPlaytimeValue = parseNumber(maxPlaytimeHours);

    return (
      (showFavoritesOnly ? 1 : 0) +
      selectedGenres.length +
      selectedPlatforms.length +
      (playedState !== "all" ? 1 : 0) +
      (installState !== "all" ? 1 : 0) +
      (playStatusState !== "all" ? 1 : 0) +
      (requireMetadata ? 1 : 0) +
      (minRatingValue !== null || maxRatingValue !== null ? 1 : 0) +
      (minMetacriticValue !== null || maxMetacriticValue !== null ? 1 : 0) +
      (minPlaytimeValue !== null || maxPlaytimeValue !== null ? 1 : 0)
    );
  }, [
    maxMetacritic,
    maxPlaytimeHours,
    maxRating,
    minMetacritic,
    minPlaytimeHours,
    minRating,
    playedState,
    installState,
    playStatusState,
    requireMetadata,
    selectedGenres.length,
    selectedPlatforms.length,
    showFavoritesOnly,
  ]);

  useEffect(() => {
    persistLibraryFilterPreset({
      searchQuery,
      viewMode,
      sortBy,
      showFavoritesOnly,
      showAdvancedFilters,
      selectedGenres,
      selectedPlatforms,
      playedState,
      installState,
      playStatusState,
      ratingMode,
      minRating,
      maxRating,
      minMetacritic,
      maxMetacritic,
      minPlaytimeHours,
      maxPlaytimeHours,
      requireMetadata,
    });
  }, [
    searchQuery,
    viewMode,
    sortBy,
    showFavoritesOnly,
    showAdvancedFilters,
    selectedGenres,
    selectedPlatforms,
    playedState,
    installState,
    playStatusState,
    ratingMode,
    minRating,
    maxRating,
    minMetacritic,
    maxMetacritic,
    minPlaytimeHours,
    maxPlaytimeHours,
    requireMetadata,
  ]);

  const { dropActive, dropHandlers } = useDropZone({
    onDropPaths: handleDroppedPaths,
  });



  const filteredGames = useMemo(() => {
    let result = (showFavoritesOnly ? favorites : games).slice();
    const minRatingValue = parseNumber(minRating);
    const maxRatingValue = parseNumber(maxRating);
    const minMetacriticValue = parseNumber(minMetacritic);
    const maxMetacriticValue = parseNumber(maxMetacritic);
    const minPlaytimeHoursValue = parseNumber(minPlaytimeHours);
    const maxPlaytimeHoursValue = parseNumber(maxPlaytimeHours);

    if (selectedGenres.length > 0) {
      const selected = new Set(selectedGenres);
      result = result.filter((game) =>
        splitList(game.genres).some((genre) => selected.has(genre)),
      );
    }

    if (selectedPlatforms.length > 0) {
      const selected = new Set(selectedPlatforms);
      result = result.filter((game) =>
        splitList(game.platforms).some((platform) => selected.has(platform)),
      );
    }

    if (playedState !== "all") {
      result = result.filter((game) => {
        const hasPlaytime = game.total_playtime > 0;
        return playedState === "played" ? hasPlaytime : !hasPlaytime;
      });
    }

    if (installState !== "all") {
      result = result.filter((game) => {
        const isInstalled = installedById[game.id] ?? true;
        return installState === "installed" ? isInstalled : !isInstalled;
      });
    }

    if (playStatusState !== "all") {
      result = result.filter((game) => game.play_status === playStatusState);
    }

    if (requireMetadata) {
      result = result.filter(
        (game) =>
          Boolean(game.description) ||
          Boolean(game.background_image) ||
          Boolean(game.platforms) ||
          Boolean(game.genres) ||
          Boolean(game.cover_image) ||
          Boolean(game.developers) ||
          Boolean(game.publishers),
      );
    }

    if (minRatingValue !== null || maxRatingValue !== null) {
      result = result.filter((game) => {
        const rating =
          ratingMode === "user" ? game.user_rating : game.metacritic;
        if (typeof rating !== "number") return false;
        if (minRatingValue !== null && rating < minRatingValue) return false;
        if (maxRatingValue !== null && rating > maxRatingValue) return false;
        return true;
      });
    }

    if (minMetacriticValue !== null || maxMetacriticValue !== null) {
      result = result.filter((game) => {
        if (typeof game.metacritic !== "number") return false;
        if (minMetacriticValue !== null && game.metacritic < minMetacriticValue)
          return false;
        if (maxMetacriticValue !== null && game.metacritic > maxMetacriticValue)
          return false;
        return true;
      });
    }

    if (minPlaytimeHoursValue !== null || maxPlaytimeHoursValue !== null) {
      result = result.filter((game) => {
        const playtimeHours = game.total_playtime / 3600;
        if (minPlaytimeHoursValue !== null && playtimeHours < minPlaytimeHoursValue)
          return false;
        if (maxPlaytimeHoursValue !== null && playtimeHours > maxPlaytimeHoursValue)
          return false;
        return true;
      });
    }

    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      result = result.filter(
        (game) =>
          game.name.toLowerCase().includes(query) ||
          game.exe_name.toLowerCase().includes(query) ||
          (game.genres?.toLowerCase().includes(query) ?? false) ||
          (game.platforms?.toLowerCase().includes(query) ?? false),
      );
    }

    return result.sort((a, b) => {
      switch (sortBy) {
        case "playtime":
          return b.total_playtime - a.total_playtime;
        case "lastPlayed":
          if (!a.last_played && !b.last_played) return 0;
          if (!a.last_played) return 1;
          if (!b.last_played) return -1;
          return (
            new Date(b.last_played).getTime() -
            new Date(a.last_played).getTime()
          );
        case "dateAdded":
          return (
            new Date(b.date_added).getTime() - new Date(a.date_added).getTime()
          );
        case "playCount":
          return b.play_count - a.play_count;
        default:
          return a.name.localeCompare(b.name);
      }
    });
  }, [
    games,
    favorites,
    searchQuery,
    sortBy,
    showFavoritesOnly,
    selectedGenres,
    selectedPlatforms,
    playedState,
    installState,
    playStatusState,
    installedById,
    requireMetadata,
    minPlaytimeHours,
    maxPlaytimeHours,
    minMetacritic,
    maxMetacritic,
    minRating,
    maxRating,
    ratingMode,
  ]);

  const currentMetadataGame = metadataQueue[0] ?? null;

  if (loading) {
    return (
      <>
        <div className="p-6">
          <div className="animate-pulse space-y-4">
            <div className="h-10 bg-muted rounded w-64" />
            <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
              {[...Array(8)].map((_, i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: Skeleton placeholders have stable order/length.
                <div key={i} className="aspect-[3/4] bg-muted rounded-lg" />
              ))}
            </div>
          </div>
        </div>

        {currentMetadataGame && (
          <RawgMetadataPrompt
            game={currentMetadataGame}
            remaining={metadataQueue.length}
            onNext={() => setMetadataQueue((prev) => prev.slice(1))}
            onSkipAll={() => setMetadataQueue([])}
            onAfterApply={refreshGames}
          />
        )}
      </>
    );
  }

  return (
    <>
      <div className="p-6 space-y-6" {...dropHandlers}>
        {/* Header */}
        <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
          <div>
            <h1 className="text-2xl font-bold tracking-tight">
              Моя библиотека
            </h1>
            <p className="text-muted-foreground text-sm">
              {games.length} {games.length === 1 ? "игра" : "игр"} в библиотеке
            </p>
          </div>

          {/* Search and filters */}
          <div className="flex items-center gap-2">
            <div className="relative flex-1 md:flex-none">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
              <Input
                placeholder="Поиск игр..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-9 w-full md:w-64"
              />
            </div>
          </div>
        </div>

        {/* Toolbar */}
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-2 border-b border-border">
          <div className="flex items-center gap-2 overflow-x-auto no-scrollbar pb-2 sm:pb-0">
            <Button
              variant={showFavoritesOnly ? "secondary" : "ghost"}
              size="sm"
              onClick={() => setShowFavoritesOnly(!showFavoritesOnly)}
              className="gap-2 flex-shrink-0"
            >
              <Star
                className={cn(
                  "w-4 h-4",
                  showFavoritesOnly && "fill-yellow-500 text-yellow-500",
                )}
              />
              Избранное
            </Button>

            <div className="h-4 w-px bg-border flex-shrink-0" />

            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="outline"
                  size="sm"
                  className="gap-2 flex-shrink-0"
                >
                  <SortAsc className="w-4 h-4" />
                  <span>Сортировка: {sortByLabel[sortBy]}</span>
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start">
                <DropdownMenuLabel>Сортировка</DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuRadioGroup
                  value={sortBy}
                  onValueChange={(value) => setSortBy(value as SortBy)}
                >
                  {Object.entries(sortByLabel).map(([value, label]) => (
                    <DropdownMenuRadioItem key={value} value={value}>
                      {label}
                    </DropdownMenuRadioItem>
                  ))}
                </DropdownMenuRadioGroup>
              </DropdownMenuContent>
            </DropdownMenu>

            <Button
              variant={showAdvancedFilters ? "secondary" : "outline"}
              size="sm"
              onClick={() => setShowAdvancedFilters((value) => !value)}
              className="gap-2 flex-shrink-0"
            >
              <Filter className="w-4 h-4" />
              <span>Фильтры</span>
              {hasActiveAdvancedFilters ? (
                <span className="ml-1 rounded-full bg-primary/15 text-primary text-[11px] px-1.5 py-0.5">
                  {activeFilterCount}
                </span>
              ) : null}
            </Button>

            <Button
              variant="outline"
              size="sm"
              onClick={clearAdvancedFilters}
              className="gap-2 flex-shrink-0"
            >
              <X className="w-4 h-4" />
              <span>Сбросить</span>
            </Button>
          </div>

          <div className="flex items-center gap-1 justify-end">
            <Button
              variant={viewMode === "grid" ? "secondary" : "ghost"}
              size="icon"
              className="w-8 h-8"
              onClick={() => setViewMode("grid")}
            >
              <Grid3X3 className="w-4 h-4" />
            </Button>
            <Button
              variant={viewMode === "list" ? "secondary" : "ghost"}
              size="icon"
              className="w-8 h-8"
              onClick={() => setViewMode("list")}
            >
              <List className="w-4 h-4" />
            </Button>
          </div>
        </div>

        {showAdvancedFilters ? (
          <div className="rounded-lg border border-border/70 bg-card/40 p-3 space-y-3">
            <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-2">
              <div>
                <div className="text-xs text-muted-foreground mb-1.5">
                  Жанры
                </div>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="outline"
                      className="w-full justify-between"
                      size="sm"
                    >
                      <span>
                        {selectedGenres.length > 0
                          ? `${selectedGenres.length} выбрано`
                          : "Все жанры"}
                      </span>
                      <Check className="w-3.5 h-3.5 opacity-70" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent className="w-full min-w-[240px]">
                    <DropdownMenuLabel>Жанры</DropdownMenuLabel>
                    <DropdownMenuSeparator />
                    {genreOptions.length === 0 ? (
                      <DropdownMenuItem disabled>Нет жанров</DropdownMenuItem>
                    ) : (
                      genreOptions.map((genre) => (
                        <DropdownMenuCheckboxItem
                          key={genre}
                          checked={selectedGenres.includes(genre)}
                          onCheckedChange={(checked) => {
                            setSelectedGenres((prev) => {
                              if (checked) {
                                return Array.from(new Set([...prev, genre]));
                              }
                              return prev.filter((value) => value !== genre);
                            });
                          }}
                        >
                          {genre}
                        </DropdownMenuCheckboxItem>
                      ))
                    )}
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>

              <div>
                <div className="text-xs text-muted-foreground mb-1.5">
                  Платформы
                </div>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="outline"
                      className="w-full justify-between"
                      size="sm"
                    >
                      <span>
                        {selectedPlatforms.length > 0
                          ? `${selectedPlatforms.length} выбрано`
                          : "Все платформы"}
                      </span>
                      <Check className="w-3.5 h-3.5 opacity-70" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent className="w-full min-w-[240px]">
                    <DropdownMenuLabel>Платформы</DropdownMenuLabel>
                    <DropdownMenuSeparator />
                    {platformOptions.length === 0 ? (
                      <DropdownMenuItem disabled>
                        Нет платформ
                      </DropdownMenuItem>
                    ) : (
                      platformOptions.map((platform) => (
                        <DropdownMenuCheckboxItem
                          key={platform}
                          checked={selectedPlatforms.includes(platform)}
                          onCheckedChange={(checked) => {
                            setSelectedPlatforms((prev) => {
                              if (checked) {
                                return Array.from(new Set([...prev, platform]));
                              }
                              return prev.filter((value) => value !== platform);
                            });
                          }}
                        >
                          {platform}
                        </DropdownMenuCheckboxItem>
                      ))
                    )}
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>

              <div>
                <div className="text-xs text-muted-foreground mb-1.5">
                  Рейтинг
                </div>
                <div className="flex items-center gap-2">
                  <Input
                    type="text"
                    inputMode="decimal"
                    placeholder="Мин."
                    value={minRating}
                    onChange={(event) => setMinRating(event.target.value)}
                    className="h-9"
                  />
                  <Input
                    type="text"
                    inputMode="decimal"
                    placeholder="Макс."
                    value={maxRating}
                    onChange={(event) => setMaxRating(event.target.value)}
                    className="h-9"
                  />
                </div>
                <div className="mt-1.5">
                  <div className="text-xs text-muted-foreground mb-1">
                    Источник
                  </div>
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button
                        variant="outline"
                        size="sm"
                        className="w-full justify-between"
                      >
                        <span>
                          {ratingMode === "user"
                            ? "Личный рейтинг"
                            : "Metacritic"}
                        </span>
                        <Check className="w-3.5 h-3.5 opacity-70" />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="start" className="w-44">
                      <DropdownMenuRadioGroup
                        value={ratingMode}
                        onValueChange={(value) =>
                          setRatingMode(value as RatingMode)
                        }
                      >
                        <DropdownMenuRadioItem value="user">
                          Личный рейтинг
                        </DropdownMenuRadioItem>
                        <DropdownMenuRadioItem value="metacritic">
                          Metacritic
                        </DropdownMenuRadioItem>
                      </DropdownMenuRadioGroup>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </div>

              <div>
                <div className="text-xs text-muted-foreground mb-1.5">
                  Metacritic (0-100)
                </div>
                <div className="flex items-center gap-2">
                  <Input
                    type="text"
                    inputMode="decimal"
                    placeholder="Мин."
                    value={minMetacritic}
                    onChange={(event) => setMinMetacritic(event.target.value)}
                    className="h-9"
                  />
                  <Input
                    type="text"
                    inputMode="decimal"
                    placeholder="Макс."
                    value={maxMetacritic}
                    onChange={(event) => setMaxMetacritic(event.target.value)}
                    className="h-9"
                  />
                </div>
              </div>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-2">
              <div>
                <div className="text-xs text-muted-foreground mb-1.5">
                  Время в игре (часы)
                </div>
                <div className="flex items-center gap-2">
                  <Input
                    type="text"
                    inputMode="decimal"
                    placeholder="Мин."
                    value={minPlaytimeHours}
                    onChange={(event) => setMinPlaytimeHours(event.target.value)}
                    className="h-9"
                  />
                  <Input
                    type="text"
                    inputMode="decimal"
                    placeholder="Макс."
                    value={maxPlaytimeHours}
                    onChange={(event) => setMaxPlaytimeHours(event.target.value)}
                    className="h-9"
                  />
                </div>
              </div>

              <div>
                <div className="text-xs text-muted-foreground mb-1.5">
                  Статус
                </div>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="outline"
                      size="sm"
                      className="w-full justify-between"
                    >
                      <span>{playedStateLabel[playedState]}</span>
                      <Check className="w-3.5 h-3.5 opacity-70" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start">
                    <DropdownMenuRadioGroup
                      value={playedState}
                      onValueChange={(value) =>
                        setPlayedState(value as PlayedState)
                      }
                    >
                      {Object.entries(playedStateLabel).map(([value, label]) => (
                        <DropdownMenuRadioItem key={value} value={value}>
                          {label}
                        </DropdownMenuRadioItem>
                      ))}
                    </DropdownMenuRadioGroup>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>

              <div>
                <div className="text-xs text-muted-foreground mb-1.5">
                  Установка
                </div>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="outline"
                      size="sm"
                      className="w-full justify-between"
                    >
                      <span>{installStateLabel[installState]}</span>
                      <Check className="w-3.5 h-3.5 opacity-70" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start">
                    <DropdownMenuRadioGroup
                      value={installState}
                      onValueChange={(value) =>
                        setInstallState(value as InstallState)
                      }
                    >
                      {Object.entries(installStateLabel).map(
                        ([value, label]) => (
                          <DropdownMenuRadioItem key={value} value={value}>
                            {label}
                          </DropdownMenuRadioItem>
                        ),
                      )}
                  </DropdownMenuRadioGroup>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>

            <div>
              <div className="text-xs text-muted-foreground mb-1.5">
                Статус прохождения
              </div>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="outline"
                    size="sm"
                    className="w-full justify-between"
                  >
                    <span>{playStatusLabel[playStatusState]}</span>
                    <Check className="w-3.5 h-3.5 opacity-70" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start">
                  <DropdownMenuRadioGroup
                    value={playStatusState}
                    onValueChange={(value) =>
                      setPlayStatusState(value as PlayStatusState)
                    }
                  >
                    {Object.entries(playStatusLabel).map(([value, label]) => (
                      <DropdownMenuRadioItem key={value} value={value}>
                        {label}
                      </DropdownMenuRadioItem>
                    ))}
                  </DropdownMenuRadioGroup>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>

              <div className="flex items-end">
                <Button
                  variant={requireMetadata ? "secondary" : "outline"}
                  size="sm"
                  onClick={() => setRequireMetadata((value) => !value)}
                  className="gap-2"
                >
                  <ListFilter className="w-4 h-4" />
                  <span>
                    {requireMetadata
                      ? "Только с метаданными"
                      : "Нужны метаданные"}
                  </span>
                </Button>
              </div>
            </div>
          </div>
        ) : null}

        {/* Empty state */}
        {filteredGames.length === 0 && (
          <div className="flex flex-col items-center justify-center py-16 text-center">
            <Gamepad2 className="w-12 h-12 text-muted-foreground mb-4" />
            {games.length === 0 ? (
              <>
                <h3 className="text-lg font-medium mb-2">Нет игр</h3>
                <p className="text-muted-foreground mb-4">
                  Просканируйте папку, чтобы добавить игры
                </p>
                <Link to="/scan">
                  <Button>Сканировать</Button>
                </Link>
              </>
            ) : (
              <>
                <h3 className="text-lg font-medium mb-2">Игры не найдены</h3>
                <p className="text-muted-foreground">
                  Попробуйте изменить поиск или фильтры
                </p>
              </>
            )}
          </div>
        )}

        {/* Game Grid/List */}
        {viewMode === "grid" ? (
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-4">
            {filteredGames.map((game) => (
              <GameCard key={game.id} game={game} />
            ))}
          </div>
        ) : (
          <div className="space-y-2">
            {filteredGames.map((game) => (
              <GameListItem key={game.id} game={game} />
            ))}
          </div>
        )}

        {currentMetadataGame && (
          <RawgMetadataPrompt
            game={currentMetadataGame}
            remaining={metadataQueue.length}
            onNext={() => setMetadataQueue((prev) => prev.slice(1))}
            onSkipAll={() => setMetadataQueue([])}
            onAfterApply={refreshGames}
          />
        )}
      </div>

      {dropActive && (
        <div className="fixed inset-0 z-50 pointer-events-none">
          <div className="absolute inset-0 bg-background/40 backdrop-blur-sm" />
          <div className="absolute inset-3 sm:inset-6 rounded-2xl border-2 border-dashed border-primary/70 shadow-[0_0_0_1px_rgba(255,255,255,0.06),0_30px_80px_rgba(8,12,24,0.55)]">
            <div className="absolute inset-0 flex items-center justify-center">
              <div className="bg-card/80 backdrop-blur-xl border border-border/60 rounded-2xl px-6 py-4 text-center shadow-[0_18px_45px_rgba(8,12,24,0.45)]">
                <div className="text-sm font-semibold text-foreground">
                  {
                    "Отпустите, чтобы добавить игру"
                  }
                </div>
                <div className="text-xs text-muted-foreground">
                  {
                    "Поддерживаются .exe и .lnk"
                  }
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

function GameListItem({ game }: { game: Game }) {
  return (
    <Link
      to={`/game/${game.id}`}
      className="flex items-center gap-3 sm:gap-4 p-2 sm:p-3 rounded-lg hover:bg-accent transition-colors group"
    >
      {/* Thumbnail */}
      <div className="w-12 h-12 sm:w-16 sm:h-16 rounded-md overflow-hidden bg-muted flex-shrink-0">
        {game.background_image ? (
          <img
            src={game.background_image}
            alt={game.name}
            className="w-full h-full object-cover"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center">
            <Gamepad2 className="w-6 h-6 text-muted-foreground" />
          </div>
        )}
      </div>

      {/* Info */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <h3 className="font-medium truncate text-sm sm:text-base">
            {game.name}
          </h3>
          {game.is_favorite && (
            <Star className="w-3 h-3 sm:w-4 sm:h-4 fill-yellow-500 text-yellow-500 flex-shrink-0" />
          )}
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs sm:text-sm text-muted-foreground">
          {game.genres && (
            <span className="truncate">{game.genres.split(",")[0]}</span>
          )}
          {game.total_playtime > 0 && (
            <span className="flex items-center gap-1">
              <Clock className="w-3 h-3" />
              {formatPlaytime(game.total_playtime)}
            </span>
          )}
          {game.last_played && (
            <span className="flex items-center gap-1 hidden sm:flex">
              <Play className="w-3 h-3" />
              {new Date(game.last_played).toLocaleDateString()}
            </span>
          )}
        </div>
      </div>

      {/* Rating */}
      {game.metacritic && (
        <div
          className={cn(
            "px-1.5 py-0.5 sm:px-2 sm:py-1 rounded text-[10px] sm:text-sm font-medium",
            game.metacritic >= 75
              ? "bg-green-500/10 text-green-500"
              : game.metacritic >= 50
                ? "bg-yellow-500/10 text-yellow-500"
                : "bg-red-500/10 text-red-500",
          )}
        >
          {game.metacritic}
        </div>
      )}
    </Link>
  );
}




