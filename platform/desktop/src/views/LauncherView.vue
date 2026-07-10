<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted, nextTick, watch } from "vue";
import type { Component } from "vue";
import {
  Settings as SettingsIcon,
  Database as DatabaseIcon,
  Clipboard as ClipboardIcon,
  Target as TargetIcon,
  ArrowLeft,
  ArrowUpCircle,
  EyeOff,
  ChevronDown,
  ListFilter,
  Loader2,
  RefreshCw,
  Check,
  Pause,
  Play,
  SkipForward,
  Square,
  Pencil,
} from "@lucide/vue";
import { KbdKey, ActionsPanel } from "@kosmos/visuals";
import BuiltInIcon from "../components/BuiltInIcon.vue";
import ClipboardQuickPanel from "../components/ClipboardQuickPanel.vue";
import FileSearchResultRow from "../components/FileSearchResultRow.vue";
import FocusCommandPanel from "../components/FocusCommandPanel.vue";
import { dedupeCommandsById } from "../lib/launcherCommands";
import {
  buildFocusAwareCommands,
  FOCUS_PAUSE_TOGGLE_ID,
  FOCUS_SKIP_ID,
  FOCUS_DONE_ID,
  FOCUS_STOP_ID,
  FOCUS_EDIT_ID,
} from "../lib/focusLauncherCommands";
import delphiSvg from "../assets/delphi.svg";
import delphiAddSvg from "../assets/delphi-add.svg";
import arraSvg from "../assets/arra.svg";
import edenSvg from "../assets/eden.svg";
import edenAddSvg from "../assets/eden-add.svg";
import edenDiarySvg from "../assets/eden-diary.svg";
import type {
  ClipboardHistoryItem,
  CommandRecord,
  FocusSessionSnapshot,
  UpdateState,
} from "@shared/ipc-types";

interface BuiltInIconConfig {
  icon?: Component;
  svgSrc?: string;
  from: string;
  to: string;
  iconColor?: string;
}

// Delphi accent gradient — sky blue.
const DELPHI_GRADIENT = {
  from: "oklch(0.78 0.14 230)",
  to: "oklch(0.5 0.18 245)",
};

// Arrancador accent gradient — crimson/red (игровая библиотека).
const ARRANCADOR_GRADIENT = {
  from: "oklch(0.7 0.2 25)",
  to: "oklch(0.45 0.18 20)",
};

// Eden accent gradient — оранжевый #FF5C00.
const EDEN_GRADIENT = {
  from: "#ff5c00",
  to: "#b33800",
};

const BUILTIN_ICONS: Record<string, BuiltInIconConfig> = {
  "settings:open": {
    icon: SettingsIcon,
    from: "oklch(0.42 0 0)",
    to: "oklch(0.26 0 0)",
  },
  "dashboard:open": {
    icon: DatabaseIcon,
    from: "oklch(0.62 0.16 165)",
    to: "oklch(0.42 0.14 175)",
  },
  "kepler:clipboard-history": {
    icon: ClipboardIcon,
    from: "oklch(0.72 0.15 260)",
    to: "oklch(0.48 0.17 270)",
  },
  "kepler:focus-session": {
    icon: TargetIcon,
    from: "oklch(0.7 0.16 145)",
    to: "oklch(0.46 0.14 165)",
  },
  "delphi:open": { svgSrc: delphiSvg, ...DELPHI_GRADIENT },
  "delphi:inbox": { svgSrc: delphiAddSvg, ...DELPHI_GRADIENT },
  "arrancador:open": { svgSrc: arraSvg, ...ARRANCADOR_GRADIENT },
  "eden:open": { svgSrc: edenSvg, ...EDEN_GRADIENT },
  "eden:note:create": { svgSrc: edenAddSvg, ...EDEN_GRADIENT },
  "eden:note:open-today": { svgSrc: edenDiarySvg, ...EDEN_GRADIENT },
  "kepler:check-updates": {
    icon: RefreshCw,
    from: "oklch(0.98 0 0)",
    to: "oklch(0.86 0 0)",
    iconColor: "oklch(0.22 0 0)",
  },
};

const FOCUS_GRADIENT = { from: "oklch(0.7 0.16 145)", to: "oklch(0.46 0.14 165)" };

function builtInIconFor(cmd: CommandRecord): BuiltInIconConfig | null {
  // Синтетические focus-команды (см. focusLauncherCommands.ts): иконки задаём
  // здесь, т.к. их нет в command bus. Toggle паузы — Play/Pause по состоянию.
  switch (cmd.id) {
    case FOCUS_PAUSE_TOGGLE_ID:
      return { icon: focusPaused.value ? Play : Pause, ...FOCUS_GRADIENT };
    case FOCUS_SKIP_ID:
      return { icon: SkipForward, ...FOCUS_GRADIENT };
    case FOCUS_DONE_ID:
      return { icon: Check, ...FOCUS_GRADIENT };
    case FOCUS_STOP_ID:
      return { icon: Square, ...FOCUS_GRADIENT };
    case FOCUS_EDIT_ID:
      return { icon: Pencil, ...FOCUS_GRADIENT };
  }
  return BUILTIN_ICONS[cmd.id] ?? null;
}

const query = ref("");
const commands = ref<CommandRecord[]>([]);
const fileCommands = ref<CommandRecord[]>([]);
const selectedIndex = ref(0);

// Состояние активной фокус-сессии — управляет тем, какие focus-команды видны в
// лаунчере (idle: «Начать фокус»; active: пауза/выполнена/завершить/редактировать).
const focusSnapshot = ref<FocusSessionSnapshot | null>(null);
const focusActive = computed(
  () => !!focusSnapshot.value && focusSnapshot.value.pomodoro.phase !== "idle",
);
const focusPaused = computed(() => !!focusSnapshot.value?.pomodoro.isPaused);
// Открыта ли focus-панель в режиме редактирования активной сессии (футер →
// «Продолжить») vs запуска новой (футер → «Начать фокус»).
const focusEditMode = ref(false);

// Команды для отображения: сырой `commands` прогоняется через состояние-зависимую
// трансформацию focus-команд. Используется во ВСЕХ местах рендера списка вместо
// `commands.value`, чтобы favorites/recent/all согласованно учитывали состояние.
const displayCommands = computed<CommandRecord[]>(() =>
  buildFocusAwareCommands(commands.value, {
    active: focusActive.value,
    paused: focusPaused.value,
  }).filter(isCommandVisible),
);

async function refreshFocusSnapshot(): Promise<void> {
  try {
    focusSnapshot.value = await window.kepler.focusSession.snapshot();
  } catch {
    /* main may not be ready — ignore */
  }
}
type LauncherMode = "commands" | "clipboard" | "focus" | "hidden";
const mode = ref<LauncherMode>("commands");
const clipboardItems = ref<ClipboardHistoryItem[]>([]);
const clipboardLoading = ref(false);
type ClipboardTypeFilter = "all" | ClipboardHistoryItem["kind"];
const clipboardTypeFilter = ref<ClipboardTypeFilter>("all");
const clipboardTypeFilterOpen = ref(false);
const inputRef = ref<HTMLInputElement | null>(null);
const listRef = ref<HTMLDivElement | null>(null);
const focusPanelRef = ref<{ start: () => Promise<void> } | null>(null);
const HIDDEN_COMMANDS_KEY = "kepler.launcher.hiddenCommandIds";
const hiddenCommandIds = ref<string[]>(loadHiddenCommandIds());

// --- App Launcher (Start Menu / UWP) ---------------------------------------
// Apps загружаются один раз на старте через `app_index.list_all` и
// merge'атся в общий список `commands` как CommandRecord c kind: "app".
// Иконка приходит уже data-URL'ом. Filter / grouping / keyboard nav —
// всё через существующую логику команд, отдельной секции «Приложения» нет.
interface AppEntry {
  id: string;
  name: string;
  exec_path: string;
  icon_path: string | null;
  icon_ref?: string | null;
  kind: string;
  source: string;
  mtime: number | null;
}
const APP_ID_PREFIX = "app:";
const FILE_ID_PREFIX = "file:";

function appToCommand(a: AppEntry): CommandRecord {
  return {
    id: `${APP_ID_PREFIX}${a.id}`,
    title: a.name,
    category: "open",
    kind: "app",
    icon: a.icon_ref ?? a.icon_path ?? undefined,
  };
}

async function fetchApps(): Promise<CommandRecord[]> {
  try {
    const res = await window.kepler.ark.request<{ apps: AppEntry[] }>("app_index.list_all", {
      limit: 500,
    });
    return (res?.apps ?? []).map(appToCommand);
  } catch (e) {
    console.warn("app_index.list_all failed", e);
    return [];
  }
}

interface FileEntry {
  path: string;
  name?: string;
  score: number;
}

function fileToCommand(file: FileEntry): CommandRecord {
  const fallbackName = file.path.split(/[\\/]/).filter(Boolean).at(-1);
  return {
    id: `${FILE_ID_PREFIX}${file.path}`,
    title: file.name?.trim() || fallbackName || "Без названия",
    subtitle: file.path,
    category: "open",
    kind: "file",
  };
}

async function searchFiles(text: string): Promise<CommandRecord[]> {
  try {
    const res = await window.kepler.ark.request<{ results: FileEntry[] }>("file_index.search", {
      query: text,
      limit: 8,
    });
    return (res?.results ?? []).map(fileToCommand);
  } catch (e) {
    console.warn("file_index.search failed", e);
    return [];
  }
}

interface ScoredCommand {
  cmd: CommandRecord;
  score: number;
}

function loadHiddenCommandIds(): string[] {
  try {
    const raw = localStorage.getItem(HIDDEN_COMMANDS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === "string") : [];
  } catch {
    return [];
  }
}

function isCommandVisible(cmd: CommandRecord): boolean {
  return !hiddenCommandIds.value.includes(cmd.id);
}

function scoreCommand(cmd: CommandRecord, q: string): number {
  if (!q) return 0;
  const ql = q.toLowerCase();
  const t = cmd.title.toLowerCase();
  const s = (cmd.subtitle ?? "").toLowerCase();
  const titleIdx = t.indexOf(ql);
  const subIdx = s.indexOf(ql);
  if (titleIdx < 0 && subIdx < 0) return -1;
  // Раньше = выше; title match лучше subtitle match.
  if (titleIdx === 0) return 1000;
  if (titleIdx > 0) return 500 - titleIdx;
  return 100 - subIdx;
}

const filteredCommands = computed<CommandRecord[]>(() => {
  const q = query.value.trim();
  if (!q) return displayCommands.value;
  return displayCommands.value
    .map<ScoredCommand>((cmd) => ({ cmd, score: scoreCommand(cmd, q) }))
    .filter((x) => x.score >= 0)
    .sort((a, b) => b.score - a.score)
    .map((x) => x.cmd);
});

const filteredClipboardItems = computed<ClipboardHistoryItem[]>(() => {
  const type = clipboardTypeFilter.value;
  const q = query.value.trim().toLocaleLowerCase("ru-RU");
  return clipboardItems.value.filter((item) => {
    if (type !== "all" && item.kind !== type) return false;
    if (!q) return true;
    return item.searchText.toLocaleLowerCase("ru-RU").includes(q);
  });
});

const selectedClipboardItem = computed<ClipboardHistoryItem | null>(
  () => filteredClipboardItems.value[selectedIndex.value] ?? null,
);

const clipboardTypeFilterLabel = computed(() =>
  clipboardTypeFilter.value === "image"
    ? "Изображения"
    : clipboardTypeFilter.value === "link"
      ? "Ссылки"
      : clipboardTypeFilter.value === "color"
        ? "Цвета"
        : clipboardTypeFilter.value === "file"
          ? "Файлы"
          : clipboardTypeFilter.value === "text"
            ? "Текст"
            : "Все типы",
);

const searchPlaceholder = computed(() =>
  mode.value === "clipboard" ? "Фильтр записей..." : "Поиск команд, приложений и файлов",
);

const headerTitle = computed(() =>
  mode.value === "focus" ? "Фокус" : mode.value === "hidden" ? "Скрытые команды" : "",
);

const RECENTS_KEY = "kepler.launcher.recents";
const RECENTS_LIMIT = 5;

// --- State restore (TTL-bound) ---------------------------------------------
// Сохраняем последние { mode, query, selectedIndex, scrollTop, savedAt } в
// localStorage. При показе launcher'а — если прошло меньше TTL минут,
// восстанавливаем. Иначе сбрасываем. TTL настраивается в Settings →
// kepler-shell-settings.json::launcherStateTtlMinutes (default 5).
const STATE_KEY = "kepler.launcher.state";
interface PersistedLauncherState {
  mode?: LauncherMode;
  query: string;
  selectedIndex: number;
  scrollTop: number;
  savedAt: number; // ms since epoch
}
const launcherStateTtlMs = ref<number>(5 * 60 * 1000);

function loadPersistedState(): PersistedLauncherState | null {
  try {
    const raw = localStorage.getItem(STATE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PersistedLauncherState;
    if (
      typeof parsed.query !== "string" ||
      typeof parsed.selectedIndex !== "number" ||
      typeof parsed.scrollTop !== "number" ||
      typeof parsed.savedAt !== "number"
    )
      return null;
    if (
      parsed.mode !== undefined &&
      parsed.mode !== "commands" &&
      parsed.mode !== "clipboard" &&
      parsed.mode !== "focus"
    )
      return null;
    return parsed;
  } catch {
    return null;
  }
}

function savePersistedState() {
  if (typeof window === "undefined") return;
  try {
    const state: PersistedLauncherState = {
      mode: mode.value,
      query: query.value,
      selectedIndex: selectedIndex.value,
      scrollTop: listRef.value?.scrollTop ?? 0,
      savedAt: Date.now(),
    };
    localStorage.setItem(STATE_KEY, JSON.stringify(state));
  } catch {
    /* storage quota / disabled — ignore */
  }
}

let saveStateTimer: ReturnType<typeof setTimeout> | null = null;
function scheduleSaveState() {
  if (saveStateTimer) clearTimeout(saveStateTimer);
  saveStateTimer = setTimeout(savePersistedState, 200);
}

function loadRecents(): string[] {
  try {
    const raw = localStorage.getItem(RECENTS_KEY);
    if (!raw) return [];
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? arr.filter((x): x is string => typeof x === "string") : [];
  } catch {
    return [];
  }
}

const recentIds = ref<string[]>(loadRecents());

const FAVORITES_KEY = "kepler.launcher.favorites";

function loadFavorites(): string[] {
  try {
    const raw = localStorage.getItem(FAVORITES_KEY);
    if (!raw) return [];
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? arr.filter((x): x is string => typeof x === "string") : [];
  } catch {
    return [];
  }
}

const favoriteIds = ref<string[]>(loadFavorites());
const actionsOpen = ref(false);
const menuOpen = ref(false);
const hiddenPanelOpen = ref(false);
const allCommandsCache = ref<CommandRecord[]>([]);

// --- Update banner state ----------------------------------------------------
const updateState = ref<UpdateState>({ kind: "idle" });
let unsubUpdateState: (() => void) | null = null;

// --- Post-update banner (one-shot после quitAndInstall) ---------------------
// Main process детектит `post-update.flag` в userData и шлёт `kepler:post-update`
// с актуальной версией. Banner перекрывает update-banner и dismiss'ится по клику.
const postUpdateVersion = ref<string | null>(null);
let unsubPostUpdate: (() => void) | null = null;
function dismissPostUpdate() {
  postUpdateVersion.value = null;
}

const updateBanner = computed<null | {
  title: string;
  description: string;
  icon: Component;
  spinning: boolean;
  clickable: boolean;
  progress?: number;
}>(() => {
  const s = updateState.value;
  if (s.kind === "downloaded") {
    return {
      title: `Обновить Kepler до ${s.version}`,
      description: "Установить новую версию и перезапустить",
      icon: ArrowUpCircle,
      spinning: false,
      clickable: true,
    };
  }
  if (s.kind === "downloading") {
    return {
      title: `Скачивается Kepler ${s.version}`,
      description: `Загружено ${Math.round(s.percent)}%. После завершения можно установить.`,
      icon: Loader2,
      spinning: true,
      clickable: false,
      progress: s.percent,
    };
  }
  if (s.kind === "available") {
    return {
      title: `Доступно обновление Kepler ${s.version}`,
      description: "Скачивается в фоне. Подожди немного.",
      icon: ArrowUpCircle,
      spinning: false,
      clickable: false,
    };
  }
  return null;
});

async function onBannerClick() {
  const b = updateBanner.value;
  if (!b?.clickable) return;
  await window.kepler.settings.update.install();
}

function recordRecent(id: string) {
  const next = [id, ...recentIds.value.filter((x) => x !== id)].slice(0, RECENTS_LIMIT);
  recentIds.value = next;
  try {
    localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
  } catch {
    /* storage quota — ignore */
  }
}

const groupedNoQuery = computed(() => {
  if (query.value.trim()) return null;
  const byId = new Map(displayCommands.value.map((c) => [c.id, c]));
  const favorites: CommandRecord[] = [];
  for (const id of favoriteIds.value) {
    const c = byId.get(id);
    if (c) favorites.push(c);
  }
  const recent: CommandRecord[] = [];
  for (const id of recentIds.value) {
    const c = byId.get(id);
    if (c) recent.push(c);
  }
  // «Все» намеренно содержит все команды — в том числе те, что уже есть в
  // других секциях. Это дублирование запрошено: список «Все» должен быть полным.
  return { favorites, recent, all: displayCommands.value };
});

const selectedCommand = computed<CommandRecord | null>(() => {
  if (mode.value !== "commands") return null;
  const row = rowAt(selectedIndex.value);
  if (!row || row.kind !== "cmd") return null;
  return row.cmd;
});

function toggleFavorite(id: string) {
  const favs = [...favoriteIds.value];
  const idx = favs.indexOf(id);
  if (idx >= 0) favs.splice(idx, 1);
  else favs.unshift(id);
  favoriteIds.value = favs;
  try {
    localStorage.setItem(FAVORITES_KEY, JSON.stringify(favs));
  } catch {
    /* ignore */
  }
}

function enterHiddenMode(): void {
  menuOpen.value = false;
  hiddenPanelOpen.value = false;
  mode.value = "hidden";
  query.value = "";
  selectedIndex.value = 0;
  savePersistedState();
}

function leaveHiddenMode(): void {
  mode.value = "commands";
  query.value = "";
  selectedIndex.value = 0;
  savePersistedState();
  void nextTick(() => {
    inputRef.value?.focus();
    inputRef.value?.select();
  });
}

function openGitHub() {
  void window.kepler.shell.openExternal("https://github.com/ksanrse");
}

function toggleCommandVisibility(id: string) {
  const hidden = [...hiddenCommandIds.value];
  const idx = hidden.indexOf(id);
  if (idx >= 0) hidden.splice(idx, 1);
  else hidden.push(id);
  hiddenCommandIds.value = hidden;
  try {
    localStorage.setItem(HIDDEN_COMMANDS_KEY, JSON.stringify(hidden));
  } catch {
    /* ignore */
  }
  selectedIndex.value = 0;
  void refreshCommands();
}

function onInput() {
  selectedIndex.value = 0;
  actionsOpen.value = false;
  scheduleSaveState();
}

function onListScroll() {
  scheduleSaveState();
}

// Клик по нефокусируемому ряду / пустому месту списка не должен уводить DOM-фокус
// на body — иначе @keydown на `.launcher` перестаёт получать события и ломается
// клавиатурная навигация/ввод. Держим фокус на поиске (click по @click ряду всё
// равно срабатывает). В focus-режиме (своя contenteditable-панель) — не вмешиваемся.
function onListMouseDown(e: MouseEvent) {
  if (mode.value !== "commands" && mode.value !== "clipboard") return;
  const target = e.target as HTMLElement | null;
  if (!target) return;
  if (target.closest('input, textarea, [contenteditable="true"]')) return;
  e.preventDefault();
  inputRef.value?.focus();
}

// Шаблон уважает «виртуальный» banner-item впереди: selectedIndex 0 — это
// banner, далее recent, далее all. flatList используется для invocation
// (banner не реальная команда, потому фильтруется).
function totalRows(): number {
  if (mode.value === "clipboard") return filteredClipboardItems.value.length;
  if (mode.value === "focus") return 0;
  const banner = updateBanner.value ? 1 : 0;
  if (groupedNoQuery.value) {
    return (
      banner +
      groupedNoQuery.value.favorites.length +
      groupedNoQuery.value.recent.length +
      groupedNoQuery.value.all.length
    );
  }
  return banner + filteredCommands.value.length + fileCommands.value.length;
}

function rowAt(
  idx: number,
): { kind: "banner" } | { kind: "cmd"; cmd: CommandRecord } | { kind: "clipboard" } | null {
  if (mode.value === "clipboard") {
    return filteredClipboardItems.value[idx] ? { kind: "clipboard" } : null;
  }
  if (mode.value === "focus") return null;
  const banner = updateBanner.value ? 1 : 0;
  if (banner && idx === 0) return { kind: "banner" };
  const i = idx - banner;
  if (groupedNoQuery.value) {
    const fav = groupedNoQuery.value.favorites;
    const rec = groupedNoQuery.value.recent;
    const all = groupedNoQuery.value.all;
    if (i < fav.length) return { kind: "cmd", cmd: fav[i]! };
    const j = i - fav.length;
    if (j < rec.length) return { kind: "cmd", cmd: rec[j]! };
    const k = j - rec.length;
    if (k < all.length) return { kind: "cmd", cmd: all[k]! };
    return null;
  }
  const c = filteredCommands.value[i] ?? fileCommands.value[i - filteredCommands.value.length];
  return c ? { kind: "cmd", cmd: c } : null;
}

async function invokeSelected() {
  const row = rowAt(selectedIndex.value);
  if (!row) return;
  if (row.kind === "clipboard") {
    await copyClipboardItem(selectedIndex.value);
    return;
  }
  if (row.kind === "banner") {
    await onBannerClick();
    return;
  }
  // Синтетические focus-команды (см. focusLauncherCommands.ts) — управление
  // активной сессией. Лаунчер НЕ закрываем: список перерисуется на новое
  // состояние (пауза↔продолжить; после stop/done вернётся «Начать фокус»).
  if (row.cmd.id === FOCUS_PAUSE_TOGGLE_ID) {
    focusSnapshot.value = focusPaused.value
      ? await window.kepler.focusSession.resume()
      : await window.kepler.focusSession.pause();
    selectedIndex.value = 0;
    return;
  }
  if (row.cmd.id === FOCUS_DONE_ID) {
    focusSnapshot.value = await window.kepler.focusSession.complete();
    selectedIndex.value = 0;
    return;
  }
  if (row.cmd.id === FOCUS_SKIP_ID) {
    focusSnapshot.value = await window.kepler.focusSession.skip();
    selectedIndex.value = 0;
    return;
  }
  if (row.cmd.id === FOCUS_STOP_ID) {
    focusSnapshot.value = await window.kepler.focusSession.stop();
    selectedIndex.value = 0;
    return;
  }
  if (row.cmd.id === FOCUS_EDIT_ID) {
    await enterFocusMode(true);
    return;
  }
  recordRecent(row.cmd.id);
  // App-команда (kind: "app" + id с префиксом `app:`) → app_index.launch.
  // Обычная command → command bus.
  //
  // Focus-блокировка приложений НЕ обрабатывается здесь: приложение
  // запускается всегда, а main-process watcher (focus-session.ts) убивает
  // процесс и показывает fullscreen-overlay — единый путь и для запуска из
  // лаунчера, и для запуска извне (taskbar / Пуск).
  if (row.cmd.id.startsWith(FILE_ID_PREFIX)) {
    const path = row.cmd.id.slice(FILE_ID_PREFIX.length);
    try {
      await window.kepler.ark.request("file_index.open", { path });
    } catch (e) {
      console.warn("file_index.open failed", e);
      return;
    }
    await window.kepler.window.hide();
  } else if (row.cmd.id.startsWith(APP_ID_PREFIX)) {
    const appId = row.cmd.id.slice(APP_ID_PREFIX.length);
    try {
      await window.kepler.ark.request("app_index.launch", { id: appId });
    } catch (e) {
      console.warn("app_index.launch failed", e);
      return;
    }
    await window.kepler.window.hide();
  } else {
    await window.kepler.commands.invoke(row.cmd.id);
  }
  query.value = "";
  selectedIndex.value = 0;
  // После успешного invoke — стираем persisted state, чтобы следующий
  // открытый launcher не восстанавливал старый query.
  try {
    localStorage.removeItem(STATE_KEY);
  } catch {
    /* ignore */
  }
}

async function refreshClipboardHistory(): Promise<void> {
  clipboardLoading.value = true;
  try {
    clipboardItems.value = await window.kepler.clipboardHistory.list();
    if (selectedIndex.value >= filteredClipboardItems.value.length) {
      selectedIndex.value = Math.max(0, filteredClipboardItems.value.length - 1);
    }
  } catch (e) {
    console.warn("clipboardHistory.list failed", e);
    clipboardItems.value = [];
  } finally {
    clipboardLoading.value = false;
  }
}

async function enterClipboardMode(): Promise<void> {
  actionsOpen.value = false;
  menuOpen.value = false;
  hiddenPanelOpen.value = false;
  mode.value = "clipboard";
  query.value = "";
  selectedIndex.value = 0;
  cancelFileSearch();
  await refreshClipboardHistory();
  await nextTick();
  inputRef.value?.focus();
  inputRef.value?.select();
  if (listRef.value) listRef.value.scrollTop = 0;
  savePersistedState();
}

function leaveClipboardMode(): void {
  mode.value = "commands";
  query.value = "";
  selectedIndex.value = 0;
  clipboardTypeFilterOpen.value = false;
  savePersistedState();
}

async function enterFocusMode(edit = false): Promise<void> {
  actionsOpen.value = false;
  menuOpen.value = false;
  hiddenPanelOpen.value = false;
  focusEditMode.value = edit;
  mode.value = "focus";
  query.value = "";
  selectedIndex.value = 0;
  cancelFileSearch();
  await nextTick();
  if (listRef.value) listRef.value.scrollTop = 0;
  savePersistedState();
}

function leaveFocusMode(): void {
  mode.value = "commands";
  query.value = "";
  selectedIndex.value = 0;
  focusEditMode.value = false;
  void refreshFocusSnapshot();
  savePersistedState();
  // Контейнер focus-панели (contenteditable) размонтируется → фокус уходит на
  // body, и @keydown на `.launcher` перестаёт получать события. Возвращаем
  // фокус на поиск, иначе стрелки/ввод мертвы. См. focus-mode UX отчёт.
  void nextTick(() => {
    inputRef.value?.focus();
    inputRef.value?.select();
  });
}

function leaveCommandMode(): void {
  if (mode.value === "clipboard") {
    leaveClipboardMode();
    return;
  }
  if (mode.value === "focus") {
    leaveFocusMode();
    return;
  }
  if (mode.value === "hidden") {
    leaveHiddenMode();
  }
}

function setClipboardTypeFilter(value: ClipboardTypeFilter): void {
  clipboardTypeFilter.value = value;
  clipboardTypeFilterOpen.value = false;
  selectedIndex.value = 0;
}

async function copyClipboardItem(index: number): Promise<void> {
  const item = filteredClipboardItems.value[index];
  if (!item) return;
  const ok = await window.kepler.clipboardHistory.copy(item.id);
  if (ok) {
    leaveClipboardMode();
    await window.kepler.window.hide();
  }
}

async function removeClipboardItem(index: number): Promise<void> {
  const item = filteredClipboardItems.value[index];
  if (!item) return;
  const ok = await window.kepler.clipboardHistory.delete(item.id);
  if (ok) {
    await refreshClipboardHistory();
    selectedIndex.value = Math.min(index, Math.max(0, filteredClipboardItems.value.length - 1));
  }
}

async function openClipboardItem(index: number): Promise<void> {
  const item = filteredClipboardItems.value[index];
  if (!item) return;
  const ok = await window.kepler.clipboardHistory.open(item.id);
  if (ok) {
    leaveClipboardMode();
    await window.kepler.window.hide();
  }
}

async function toggleClipboardPin(index: number): Promise<void> {
  const item = filteredClipboardItems.value[index];
  if (!item) return;
  const ok = await window.kepler.clipboardHistory.togglePin(item.id);
  if (ok) await refreshClipboardHistory();
}

async function clearClipboardHistory(): Promise<void> {
  await window.kepler.clipboardHistory.clear();
  selectedIndex.value = 0;
  await refreshClipboardHistory();
}

async function clearAllClipboardHistory(): Promise<void> {
  await window.kepler.clipboardHistory.clearAll();
  selectedIndex.value = 0;
  await refreshClipboardHistory();
}

const SCROLL_EDGE_PADDING = 8;

function moveSelection(delta: number) {
  const n = totalRows();
  if (n === 0) return;
  const prevIdx = selectedIndex.value;
  const next = prevIdx + delta;
  // Clamp без wrap — упереться в границы.
  selectedIndex.value = Math.max(0, Math.min(n - 1, next));
  scheduleSaveState();
  void nextTick(() => {
    const list = listRef.value;
    if (!list) return;
    // Для самого верхнего ряда (banner / первый item) — упираемся в top.
    if (selectedIndex.value === 0) {
      list.scrollTop = 0;
      return;
    }
    const selectedEl = list.querySelector<HTMLElement>(".result.selected");
    if (!selectedEl) return;
    // Когда выделение — первый <li> в своей <ul>, прокручиваем к заголовку
    // секции (sibling <ul> → previousElementSibling = .section-label).
    const isFirstInUl = selectedEl.parentElement?.firstElementChild === selectedEl;
    if (isFirstInUl) {
      const header = selectedEl.parentElement!.previousElementSibling as HTMLElement | null;
      if (header?.classList.contains("section-label")) {
        header.scrollIntoView({ block: "start" });
        return;
      }
    }
    // Custom scrollIntoView с 8px padding к краю.
    // Проверяем ОБА overflow независимо от direction — иначе если selection
    // ушёл за пределы viewport (например, list был прокручен с прошлого
    // открытия), нажатие ArrowDown не подтянет scroll если выделенный
    // элемент оказался выше viewport.
    const listRect = list.getBoundingClientRect();
    const elRect = selectedEl.getBoundingClientRect();
    const topOverflow = listRect.top + SCROLL_EDGE_PADDING - elRect.top;
    const bottomOverflow = elRect.bottom - (listRect.bottom - SCROLL_EDGE_PADDING);
    if (topOverflow > 0) {
      list.scrollTop -= topOverflow;
    } else if (bottomOverflow > 0) {
      list.scrollTop += bottomOverflow;
    }
  });
}

function onKey(e: KeyboardEvent) {
  if (mode.value === "focus") {
    if (e.key === "Escape") {
      e.preventDefault();
      leaveFocusMode();
    }
    return;
  }
  if (mode.value === "hidden") {
    if (e.key === "Escape") {
      e.preventDefault();
      leaveHiddenMode();
    }
    return;
  }
  if (mode.value === "clipboard") {
    if (e.key === "Escape") {
      e.preventDefault();
      if (clipboardTypeFilterOpen.value) {
        clipboardTypeFilterOpen.value = false;
        return;
      }
      leaveClipboardMode();
      void window.kepler.window.hide();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      moveSelection(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      moveSelection(-1);
    } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.code === "KeyX") {
      e.preventDefault();
      void clearAllClipboardHistory();
    } else if ((e.ctrlKey || e.metaKey) && e.code === "KeyX") {
      e.preventDefault();
      void removeClipboardItem(selectedIndex.value);
    } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.code === "KeyP") {
      e.preventDefault();
      void toggleClipboardPin(selectedIndex.value);
    } else if ((e.ctrlKey || e.metaKey) && e.code === "KeyO") {
      e.preventDefault();
      void openClipboardItem(selectedIndex.value);
    } else if (e.key === "Enter") {
      e.preventDefault();
      void copyClipboardItem(selectedIndex.value);
    } else if (e.key === "Delete") {
      e.preventDefault();
      void removeClipboardItem(selectedIndex.value);
    }
    return;
  }
  if (e.key === "Escape") {
    e.preventDefault();
    if (actionsOpen.value) {
      actionsOpen.value = false;
      return;
    }
    void window.kepler.window.hide();
  } else if (e.key === "ArrowDown") {
    e.preventDefault();
    moveSelection(1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    moveSelection(-1);
  } else if (e.key === "Enter") {
    e.preventDefault();
    void invokeSelected();
  } else if ((e.ctrlKey || e.metaKey) && e.code === "KeyK") {
    e.preventDefault();
    if (selectedCommand.value) actionsOpen.value = !actionsOpen.value;
  } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.code === "KeyF") {
    e.preventDefault();
    if (selectedCommand.value) {
      toggleFavorite(selectedCommand.value.id);
      actionsOpen.value = false;
    }
  } else if ((e.ctrlKey || e.metaKey) && e.code === "KeyH") {
    e.preventDefault();
    if (selectedCommand.value && selectedCommand.value.kind !== "file") {
      toggleCommandVisibility(selectedCommand.value.id);
      actionsOpen.value = false;
    }
  }
}

let commandsRefreshRun = 0;
async function refreshCommands() {
  hiddenCommandIds.value = loadHiddenCommandIds();
  const run = ++commandsRefreshRun;
  // Двухфазно: команды (built-in появляются МГНОВЕННО) отдельно от apps. При
  // мёртвом backend'е `app_index.list_all` висит ~30s на ark-таймауте — нельзя
  // блокировать на нём показ встроенных команд (иначе лаунчер выглядит пустым).
  // Ранее известные приложения — чтобы фаза 1 НЕ схлопывала список до одних
  // built-in команд при тёплом переоткрытии (иначе apps пропадают и тут же
  // возвращаются → визуальный «прыжок», ощущение перезагрузки с нуля).
  const prevApps = allCommandsCache.value.filter((c) => c.kind === "app");
  const cmds = await window.kepler.commands.list().catch((e) => {
    console.warn("commands.list failed", e);
    return [] as CommandRecord[];
  });
  if (run !== commandsRefreshRun) return; // более свежий refresh победил
  // Фаза 1: built-in/extension команды появляются мгновенно, но уже известные
  // apps сохраняем — список не теряет элементы между открытиями.
  const phase1 = dedupeCommandsById([...cmds, ...prevApps]);
  allCommandsCache.value = phase1;
  commands.value = phase1;
  // Фаза 2: свежие приложения (могут быть медленными/пустыми при мёртвом backend).
  const apps = await fetchApps();
  if (run !== commandsRefreshRun || apps.length === 0) return;
  const deduped = dedupeCommandsById([...cmds, ...apps]);
  allCommandsCache.value = deduped;
  commands.value = deduped;
}

const hiddenCommandsList = computed<CommandRecord[]>(() =>
  buildFocusAwareCommands(allCommandsCache.value, {
    active: focusActive.value,
    paused: focusPaused.value,
  }).filter((cmd) => hiddenCommandIds.value.includes(cmd.id)),
);

let offShow = () => {};
let offCommandsUpdated = () => {};
let offClipboardOpen = () => {};
let offClipboardUpdated = () => {};
let offFocusOpen = () => {};
let offFocusSessionUpdated = () => {};
let offCommandVisibilityStorage = () => {};
let offBackendReady = () => {};
let offHide = () => {};
let fileSearchTimer: ReturnType<typeof setTimeout> | null = null;
let fileSearchRun = 0;
const FILE_SEARCH_DEBOUNCE_MS = 120;
const MIN_FILE_SEARCH_QUERY_CHARS = 3;

function cancelFileSearch(): void {
  fileSearchRun++;
  if (fileSearchTimer) {
    clearTimeout(fileSearchTimer);
    fileSearchTimer = null;
  }
  fileCommands.value = [];
}

function scheduleFileSearch(text: string, run: number, delay: number) {
  if (fileSearchTimer) clearTimeout(fileSearchTimer);
  fileSearchTimer = setTimeout(async () => {
    fileSearchTimer = null;
    const found = await searchFiles(text);
    if (run === fileSearchRun) {
      fileCommands.value = found;
    }
  }, delay);
}

watch(query, (value) => {
  if (mode.value !== "commands") {
    selectedIndex.value = 0;
    cancelFileSearch();
    return;
  }
  const text = value.trim();
  if (text.length < MIN_FILE_SEARCH_QUERY_CHARS) {
    // См. postmortems.md § 2026-06-08: короткие query не должны запускать
    // backend file LIKE scan, а hidden launcher обязан оставаться тихим.
    cancelFileSearch();
    return;
  }
  const run = ++fileSearchRun;
  scheduleFileSearch(text, run, FILE_SEARCH_DEBOUNCE_MS);
});

onMounted(async () => {
  offShow = window.kepler.window.onShow(() => {
    // Восстанавливаем state если прошло меньше TTL с последнего сохранения.
    // Иначе ресетим query / selection / scroll.
    const persisted = loadPersistedState();
    const fresh =
      persisted &&
      launcherStateTtlMs.value > 0 &&
      Date.now() - persisted.savedAt <= launcherStateTtlMs.value;
    if (fresh && persisted) {
      mode.value = persisted.mode ?? "commands";
      query.value = persisted.query;
      selectedIndex.value = persisted.selectedIndex;
      if (mode.value === "clipboard") void refreshClipboardHistory();
    } else {
      mode.value = "commands";
      query.value = "";
      selectedIndex.value = 0;
    }
    void refreshCommands();
    void refreshFocusSnapshot();
    void nextTick(() => {
      if (mode.value !== "focus") {
        inputRef.value?.focus();
        inputRef.value?.select();
      }
      if (listRef.value) {
        listRef.value.scrollTop = fresh && persisted ? persisted.scrollTop : 0;
      }
    });
  });
  offCommandsUpdated = window.kepler.commands.onUpdated(() => {
    void refreshCommands();
  });
  // Backend переподнялся (recovery после сна / restart) — перезапрашиваем
  // команды и приложения, иначе список остаётся пустым до следующего show.
  offBackendReady = window.kepler.backend.onReady(() => {
    void refreshCommands();
  });
  offHide = window.kepler.window.onHide(() => {
    cancelFileSearch();
  });
  offClipboardOpen = window.kepler.clipboardHistory.onOpenShell(() => {
    void enterClipboardMode();
  });
  offClipboardUpdated = window.kepler.clipboardHistory.onUpdated(() => {
    if (mode.value === "clipboard") void refreshClipboardHistory();
  });
  offFocusOpen = window.kepler.focusSession.onOpenShell(() => {
    void enterFocusMode();
  });
  // Состояние сессии могло измениться извне (виджет: старт/пауза/стоп) пока
  // лаунчер открыт — пересобираем focus-команды.
  offFocusSessionUpdated = window.kepler.focusSession.onUpdated(() => {
    void refreshFocusSnapshot();
  });
  void refreshFocusSnapshot();
  const onStorage = (event: StorageEvent) => {
    if (event.key !== HIDDEN_COMMANDS_KEY) return;
    hiddenCommandIds.value = loadHiddenCommandIds();
    void refreshCommands();
  };
  window.addEventListener("storage", onStorage);
  offCommandVisibilityStorage = () => window.removeEventListener("storage", onStorage);
  try {
    updateState.value = await window.kepler.settings.update.state();
  } catch {
    /* main may not be ready yet — ignore */
  }
  unsubUpdateState = window.kepler.settings.update.onStateChanged((s) => {
    updateState.value = s;
  });
  unsubPostUpdate = window.kepler.postUpdate.onShown((payload) => {
    postUpdateVersion.value = payload.version;
  });
  // TTL для restore (минуты → мс). Если settings API недоступен — default 5 мин.
  try {
    const minutes = await window.kepler.settings.launcherStateTtl.get();
    launcherStateTtlMs.value = Math.max(0, minutes) * 60 * 1000;
  } catch {
    /* ignore */
  }
  void refreshCommands();
  void nextTick(() => inputRef.value?.focus());
});

onUnmounted(() => {
  offShow();
  offCommandsUpdated();
  offClipboardOpen();
  offClipboardUpdated();
  offFocusSessionUpdated();
  offFocusOpen();
  offCommandVisibilityStorage();
  offBackendReady();
  offHide();
  cancelFileSearch();
  unsubUpdateState?.();
  unsubPostUpdate?.();
});
</script>

<template>
  <div class="launcher" @keydown="onKey">
    <div
      class="search-bar"
      :class="{
        'search-bar--subpage': mode !== 'commands',
        'search-bar--clipboard': mode === 'clipboard',
        'search-bar--hidden': mode === 'hidden',
      }"
    >
      <button
        v-if="mode !== 'commands'"
        class="search-icon-button"
        type="button"
        title="Назад"
        @click="leaveCommandMode"
      >
        <ArrowLeft :size="17" />
      </button>
      <input
        v-if="mode !== 'focus' && mode !== 'hidden'"
        ref="inputRef"
        v-model="query"
        class="search"
        type="text"
        :placeholder="searchPlaceholder"
        spellcheck="false"
        autocomplete="off"
        autocorrect="off"
        autocapitalize="off"
        @input="onInput"
      />
      <div v-else class="search-heading">{{ headerTitle }}</div>
      <button
        v-if="mode === 'clipboard'"
        class="type-filter-button"
        type="button"
        title="Фильтр типа"
        :aria-expanded="clipboardTypeFilterOpen"
        @click="clipboardTypeFilterOpen = !clipboardTypeFilterOpen"
      >
        <ListFilter :size="17" />
        <span>{{ clipboardTypeFilterLabel }}</span>
        <ChevronDown :size="14" />
      </button>
      <div v-if="mode === 'clipboard' && clipboardTypeFilterOpen" class="type-filter-menu">
        <button
          class="type-filter-menu__item"
          :class="{ 'type-filter-menu__item--active': clipboardTypeFilter === 'all' }"
          type="button"
          @click="setClipboardTypeFilter('all')"
        >
          Все типы
        </button>
        <button
          class="type-filter-menu__item"
          :class="{ 'type-filter-menu__item--active': clipboardTypeFilter === 'text' }"
          type="button"
          @click="setClipboardTypeFilter('text')"
        >
          Текст
        </button>
        <button
          class="type-filter-menu__item"
          :class="{ 'type-filter-menu__item--active': clipboardTypeFilter === 'image' }"
          type="button"
          @click="setClipboardTypeFilter('image')"
        >
          Изображения
        </button>
        <button
          class="type-filter-menu__item"
          :class="{ 'type-filter-menu__item--active': clipboardTypeFilter === 'link' }"
          type="button"
          @click="setClipboardTypeFilter('link')"
        >
          Ссылки
        </button>
        <button
          class="type-filter-menu__item"
          :class="{ 'type-filter-menu__item--active': clipboardTypeFilter === 'color' }"
          type="button"
          @click="setClipboardTypeFilter('color')"
        >
          Цвета
        </button>
        <button
          class="type-filter-menu__item"
          :class="{ 'type-filter-menu__item--active': clipboardTypeFilter === 'file' }"
          type="button"
          @click="setClipboardTypeFilter('file')"
        >
          Файлы
        </button>
      </div>
    </div>
    <div
      ref="listRef"
      class="list kosmos-scroll"
      :class="{ 'list--clipboard': mode === 'clipboard', 'list--focus': mode === 'focus' }"
      @scroll="onListScroll"
      @mousedown="onListMouseDown"
    >
      <ClipboardQuickPanel
        v-if="mode === 'clipboard'"
        :items="filteredClipboardItems"
        :selected-item="selectedClipboardItem"
        :selected-index="selectedIndex"
        :loading="clipboardLoading"
        :empty-label="clipboardItems.length === 0 ? 'История пока пустая' : 'Ничего не найдено'"
        @select="selectedIndex = $event"
        @copy="copyClipboardItem"
        @open="openClipboardItem"
        @toggle-pin="toggleClipboardPin"
        @remove="removeClipboardItem"
        @clear="clearClipboardHistory"
        @clear-all="clearAllClipboardHistory"
      />
      <FocusCommandPanel v-else-if="mode === 'focus'" ref="focusPanelRef" />
      <template v-else-if="mode === 'hidden'">
        <div v-if="hiddenCommandsList.length === 0" class="empty">Нет скрытых команд</div>
        <ul v-else class="results">
          <li v-for="cmd in hiddenCommandsList" :key="cmd.id" class="result result--hidden">
            <BuiltInIcon
              v-if="builtInIconFor(cmd)"
              :icon="builtInIconFor(cmd)!.icon"
              :svg-src="builtInIconFor(cmd)!.svgSrc"
              :icon-color="builtInIconFor(cmd)!.iconColor"
              :from="builtInIconFor(cmd)!.from"
              :to="builtInIconFor(cmd)!.to"
            />
            <img
              v-else-if="cmd.icon"
              :src="cmd.icon"
              class="icon"
              alt=""
              loading="lazy"
              decoding="async"
            />
            <BuiltInIcon v-else />
            <span class="title">{{ cmd.title }}</span>
            <span class="kind-label">{{
              cmd.appName ?? (cmd.kind === "app" ? "Приложение" : "Команда")
            }}</span>
            <button type="button" class="unhide-btn" @click.stop="toggleCommandVisibility(cmd.id)">
              Показать
            </button>
          </li>
        </ul>
      </template>
      <template v-else>
        <template v-if="postUpdateVersion">
          <div class="section-label">Готово</div>
          <ul class="results">
            <li class="result update-tile post-update-tile" @click="dismissPostUpdate">
              <span class="update-icon update-icon-large post-update-icon">
                <Check :size="22" :stroke-width="2.5" />
              </span>
              <div class="update-body">
                <div class="update-title">Kepler обновлён до v{{ postUpdateVersion }}</div>
                <div class="update-description">Нажми, чтобы скрыть</div>
              </div>
            </li>
          </ul>
        </template>
        <template v-else-if="updateBanner">
          <div class="section-label">Обновление</div>
          <ul class="results">
            <li
              class="result update-tile"
              :class="{ selected: selectedIndex === 0, disabled: !updateBanner.clickable }"
              @click="
                () => {
                  selectedIndex = 0;
                  void invokeSelected();
                }
              "
            >
              <span class="update-icon update-icon-large">
                <component
                  :is="updateBanner.icon"
                  :size="22"
                  :stroke-width="2"
                  :class="{ spin: updateBanner.spinning }"
                />
              </span>
              <div class="update-body">
                <div class="update-title">{{ updateBanner.title }}</div>
                <div class="update-description">{{ updateBanner.description }}</div>
              </div>
              <span
                v-if="updateBanner.progress !== undefined"
                class="update-tile-progress"
                :style="{ width: `${updateBanner.progress}%` }"
              />
            </li>
          </ul>
        </template>
        <template v-if="groupedNoQuery">
          <template v-if="groupedNoQuery.favorites.length > 0">
            <div class="section-label">Избранное</div>
            <ul class="results">
              <li
                v-for="(cmd, idx) in groupedNoQuery.favorites"
                :key="`fav-${cmd.id}`"
                class="result"
                :class="{ selected: (updateBanner ? 1 : 0) + idx === selectedIndex }"
                @click="selectedIndex = (updateBanner ? 1 : 0) + idx"
                @dblclick="
                  () => {
                    selectedIndex = (updateBanner ? 1 : 0) + idx;
                    void invokeSelected();
                  }
                "
              >
                <BuiltInIcon
                  v-if="builtInIconFor(cmd)"
                  :icon="builtInIconFor(cmd)!.icon"
                  :svg-src="builtInIconFor(cmd)!.svgSrc"
                  :icon-color="builtInIconFor(cmd)!.iconColor"
                  :from="builtInIconFor(cmd)!.from"
                  :to="builtInIconFor(cmd)!.to"
                />
                <img
                  v-else-if="cmd.icon"
                  :src="cmd.icon"
                  class="icon"
                  alt=""
                  loading="lazy"
                  decoding="async"
                />
                <BuiltInIcon v-else />
                <span class="title">{{ cmd.title }}</span>
                <span v-if="cmd.appName && cmd.kind === 'command'" class="app-name">{{
                  cmd.appName
                }}</span>
                <span class="kind-label">{{
                  cmd.kind === "command" ? "Команда" : "Приложение"
                }}</span>
              </li>
            </ul>
          </template>
          <template v-if="groupedNoQuery.recent.length > 0">
            <div class="section-label">Недавние</div>
            <ul class="results">
              <li
                v-for="(cmd, idx) in groupedNoQuery.recent"
                :key="`recent-${cmd.id}`"
                class="result"
                :class="{
                  selected:
                    (updateBanner ? 1 : 0) + groupedNoQuery.favorites.length + idx ===
                    selectedIndex,
                }"
                @click="
                  selectedIndex = (updateBanner ? 1 : 0) + groupedNoQuery!.favorites.length + idx
                "
                @dblclick="
                  () => {
                    selectedIndex = (updateBanner ? 1 : 0) + groupedNoQuery!.favorites.length + idx;
                    void invokeSelected();
                  }
                "
              >
                <BuiltInIcon
                  v-if="builtInIconFor(cmd)"
                  :icon="builtInIconFor(cmd)!.icon"
                  :svg-src="builtInIconFor(cmd)!.svgSrc"
                  :icon-color="builtInIconFor(cmd)!.iconColor"
                  :from="builtInIconFor(cmd)!.from"
                  :to="builtInIconFor(cmd)!.to"
                />
                <img
                  v-else-if="cmd.icon"
                  :src="cmd.icon"
                  class="icon"
                  alt=""
                  loading="lazy"
                  decoding="async"
                />
                <BuiltInIcon v-else />
                <span class="title">{{ cmd.title }}</span>
                <span v-if="cmd.kind === 'file' && cmd.subtitle" class="subtitle">{{
                  cmd.subtitle
                }}</span>
                <span v-if="cmd.appName && cmd.kind === 'command'" class="app-name">{{
                  cmd.appName
                }}</span>
                <span class="kind-label">{{
                  cmd.kind === "command" ? "Команда" : "Приложение"
                }}</span>
              </li>
            </ul>
          </template>
          <template v-if="groupedNoQuery.all.length > 0">
            <div class="section-label">Все</div>
            <ul class="results">
              <li
                v-for="(cmd, idx) in groupedNoQuery.all"
                :key="`all-${cmd.id}`"
                class="result"
                :class="{
                  selected:
                    (updateBanner ? 1 : 0) +
                      groupedNoQuery.favorites.length +
                      groupedNoQuery.recent.length +
                      idx ===
                    selectedIndex,
                }"
                @click="
                  selectedIndex =
                    (updateBanner ? 1 : 0) +
                    groupedNoQuery!.favorites.length +
                    groupedNoQuery!.recent.length +
                    idx
                "
                @dblclick="
                  () => {
                    selectedIndex =
                      (updateBanner ? 1 : 0) +
                      groupedNoQuery!.favorites.length +
                      groupedNoQuery!.recent.length +
                      idx;
                    void invokeSelected();
                  }
                "
              >
                <BuiltInIcon
                  v-if="builtInIconFor(cmd)"
                  :icon="builtInIconFor(cmd)!.icon"
                  :svg-src="builtInIconFor(cmd)!.svgSrc"
                  :icon-color="builtInIconFor(cmd)!.iconColor"
                  :from="builtInIconFor(cmd)!.from"
                  :to="builtInIconFor(cmd)!.to"
                />
                <img
                  v-else-if="cmd.icon"
                  :src="cmd.icon"
                  class="icon"
                  alt=""
                  loading="lazy"
                  decoding="async"
                />
                <BuiltInIcon v-else />
                <span class="title">{{ cmd.title }}</span>
                <span v-if="cmd.kind === 'file' && cmd.subtitle" class="subtitle">{{
                  cmd.subtitle
                }}</span>
                <span v-if="cmd.appName && cmd.kind === 'command'" class="app-name">{{
                  cmd.appName
                }}</span>
                <span class="kind-label">{{
                  cmd.kind === "command" ? "Команда" : "Приложение"
                }}</span>
              </li>
            </ul>
          </template>
        </template>
        <template v-else>
          <div v-if="filteredCommands.length === 0 && fileCommands.length === 0" class="empty">
            Ничего не найдено
          </div>
          <template v-if="filteredCommands.length > 0">
            <div class="section-label">Все</div>
            <ul class="results">
              <li
                v-for="(cmd, idx) in filteredCommands"
                :key="cmd.id"
                class="result"
                :class="{ selected: (updateBanner ? 1 : 0) + idx === selectedIndex }"
                @click="selectedIndex = (updateBanner ? 1 : 0) + idx"
                @dblclick="
                  () => {
                    selectedIndex = (updateBanner ? 1 : 0) + idx;
                    void invokeSelected();
                  }
                "
              >
                <BuiltInIcon
                  v-if="builtInIconFor(cmd)"
                  :icon="builtInIconFor(cmd)!.icon"
                  :svg-src="builtInIconFor(cmd)!.svgSrc"
                  :icon-color="builtInIconFor(cmd)!.iconColor"
                  :from="builtInIconFor(cmd)!.from"
                  :to="builtInIconFor(cmd)!.to"
                />
                <img
                  v-else-if="cmd.icon"
                  :src="cmd.icon"
                  class="icon"
                  alt=""
                  loading="lazy"
                  decoding="async"
                />
                <span v-else class="icon icon-placeholder" aria-hidden="true" />
                <span class="title">{{ cmd.title }}</span>
                <span v-if="cmd.appName && cmd.kind === 'command'" class="app-name">{{
                  cmd.appName
                }}</span>
                <span class="kind-label">{{
                  cmd.kind === "command" ? "Команда" : "Приложение"
                }}</span>
              </li>
            </ul>
          </template>
          <template v-if="fileCommands.length > 0">
            <div class="section-label">Файлы</div>
            <ul class="results">
              <template v-for="(cmd, idx) in fileCommands" :key="cmd.id">
                <FileSearchResultRow
                  :title="cmd.title"
                  :path="cmd.subtitle ?? ''"
                  :selected="
                    (updateBanner ? 1 : 0) + filteredCommands.length + idx === selectedIndex
                  "
                  @select="
                    () => {
                      selectedIndex = (updateBanner ? 1 : 0) + filteredCommands.length + idx;
                    }
                  "
                />
              </template>
            </ul>
          </template>
        </template>
      </template>
    </div>
    <div
      v-if="actionsOpen || menuOpen"
      class="actions-backdrop"
      @click="
        actionsOpen = false;
        menuOpen = false;
      "
    />

    <!-- Меню гамбургера -->
    <div v-if="menuOpen" class="launcher-menu">
      <ul class="launcher-menu__list">
        <li
          class="launcher-menu__item"
          @click="
            () => {
              menuOpen = false;
              openGitHub();
            }
          "
        >
          <span class="launcher-menu__label">На GitHub</span>
          <span class="launcher-menu__hint">↗</span>
        </li>
        <li class="launcher-menu__item" @click="enterHiddenMode">
          <span class="launcher-menu__label">Показать скрытые</span>
          <span v-if="hiddenCommandIds.length > 0" class="launcher-menu__badge">{{
            hiddenCommandIds.length
          }}</span>
        </li>
      </ul>
    </div>
    <ActionsPanel
      v-if="actionsOpen && selectedCommand"
      :is-favorite="favoriteIds.includes(selectedCommand.id)"
      :is-hidden="hiddenCommandIds.includes(selectedCommand.id)"
      :can-hide="selectedCommand.kind !== 'file'"
      @open="
        () => {
          actionsOpen = false;
          void invokeSelected();
        }
      "
      @toggle-favorite="
        () => {
          if (selectedCommand) {
            toggleFavorite(selectedCommand.id);
            actionsOpen = false;
          }
        }
      "
      @toggle-hide="
        () => {
          if (selectedCommand) {
            toggleCommandVisibility(selectedCommand.id);
            actionsOpen = false;
          }
        }
      "
    />
    <!-- Footer: hidden mode -->
    <div v-if="mode === 'hidden'" class="launcher-footer">
      <span class="launcher-footer__left launcher-footer__left--mode">
        <EyeOff :size="14" />
        Скрытые команды
      </span>
    </div>

    <!-- Footer: clipboard mode -->
    <div v-else-if="mode === 'clipboard'" class="launcher-footer">
      <span class="launcher-footer__left launcher-footer__left--mode">
        <ClipboardIcon :size="14" />
        Буфер обмена
      </span>
      <div class="launcher-footer__actions">
        <button
          type="button"
          class="launcher-footer__hint-btn launcher-footer__hint-btn--primary"
          @click="void copyClipboardItem(selectedIndex)"
        >
          Отправить <KbdKey>↵</KbdKey>
        </button>
        <span class="launcher-footer__sep" aria-hidden="true" />
        <button
          type="button"
          class="launcher-footer__hint-btn"
          @click="void openClipboardItem(selectedIndex)"
        >
          Открыть <KbdKey>Ctrl</KbdKey><KbdKey>O</KbdKey>
        </button>
        <span class="launcher-footer__sep" aria-hidden="true" />
        <button
          type="button"
          class="launcher-footer__hint-btn"
          @click="void removeClipboardItem(selectedIndex)"
        >
          Удалить <KbdKey>Ctrl</KbdKey><KbdKey>X</KbdKey>
        </button>
      </div>
    </div>

    <!-- Footer: focus mode -->
    <div v-else-if="mode === 'focus'" class="launcher-footer">
      <span class="launcher-footer__left launcher-footer__left--mode">
        <TargetIcon :size="14" />
        Фокус
      </span>
      <button
        type="button"
        class="launcher-footer__hint-btn launcher-footer__hint-btn--primary"
        @click="void focusPanelRef?.start()"
      >
        {{ focusEditMode ? "Продолжить" : "Начать фокус" }} <KbdKey>↵</KbdKey>
      </button>
    </div>

    <!-- Footer: commands mode -->
    <div v-else class="launcher-footer">
      <button type="button" class="launcher-footer__menu-btn" @click="menuOpen = !menuOpen">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
          <rect x="1" y="3" width="12" height="1.5" rx="0.75" fill="currentColor" />
          <rect x="1" y="6.25" width="12" height="1.5" rx="0.75" fill="currentColor" />
          <rect x="1" y="9.5" width="12" height="1.5" rx="0.75" fill="currentColor" />
        </svg>
      </button>
      <div class="launcher-footer__actions">
        <button type="button" class="launcher-footer__hint-btn" @click="void invokeSelected()">
          Открыть команды <KbdKey>↵</KbdKey>
        </button>
        <span class="launcher-footer__sep" aria-hidden="true" />
        <button
          type="button"
          class="launcher-footer__hint-btn"
          @click="selectedCommand && (actionsOpen = !actionsOpen)"
        >
          Опции <KbdKey>Ctrl</KbdKey><KbdKey>K</KbdKey>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.launcher {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-app, #1d1d1f);
}

.search-bar {
  display: flex;
  align-items: center;
  height: 64px;
  flex-shrink: 0;
}

.search-bar--subpage {
  position: relative;
  gap: 10px;
  border-bottom: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  padding: 0 14px;
}

.search {
  min-width: 0;
  flex: 1;
  height: 100%;
  padding: 0 22px;
  border: none;
  outline: none;
  background: transparent;
  color: var(--foreground);
  font-size: 18px;
  font-weight: 400;
}

.search-bar--subpage .search {
  padding: 0;
}

.search-heading {
  display: flex;
  min-width: 0;
  flex: 1;
  align-items: center;
  color: var(--foreground);
  font-size: 16px;
  font-weight: 750;
}

.search-icon-button,
.type-filter-button {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 34px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
  color: color-mix(in srgb, var(--foreground) 78%, transparent);
}

.search-icon-button {
  width: 34px;
  flex: 0 0 auto;
}

.type-filter-button {
  min-width: 126px;
  justify-content: space-between;
  gap: 8px;
  padding: 0 11px;
  font-size: 13px;
  font-weight: 650;
}

.type-filter-menu {
  position: absolute;
  top: 52px;
  right: 14px;
  z-index: 10;
  display: grid;
  width: 150px;
  gap: 2px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  border-radius: 7px;
  background: color-mix(in srgb, var(--background) 92%, var(--foreground) 8%);
  padding: 5px;
  box-shadow: 0 16px 36px color-mix(in srgb, var(--background) 52%, transparent);
}

.type-filter-menu__item {
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--foreground);
  padding: 7px 9px;
  text-align: left;
  font-size: 12px;
}

.type-filter-menu__item:hover,
.type-filter-menu__item--active {
  background: color-mix(in srgb, var(--foreground) 9%, transparent);
}

.search::placeholder {
  color: color-mix(in srgb, var(--foreground) 36%, transparent);
}

.list {
  flex: 1;
  overflow-y: auto;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  padding: 8px 0;
  scrollbar-gutter: stable both-edges;
}

.list--clipboard,
.list--focus {
  border-top: 0;
  padding: 0;
}

.section-label {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
  padding: 4px 16px 8px;
}

.results {
  margin: 0;
  padding: 0;
  list-style: none;
}

.result {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border-radius: 6px;
  border: 1px solid transparent;
  background: transparent;
  margin: 1px 0;
  /* Без transition: выделение должно срабатывать моментально. */
  /* Длинный список (сотни приложений) не виртуализирован: content-visibility
     позволяет браузеру пропускать layout/paint строк вне вьюпорта — убирает
     лаг скролла. contain-intrinsic-size — размер-заглушка (≈высота строки),
     чтобы scrollbar и геометрия не прыгали. */
  content-visibility: auto;
  contain-intrinsic-size: auto 42px;
}

.icon {
  width: 20px;
  height: 20px;
  border-radius: 4px;
  flex-shrink: 0;
  object-fit: cover;
}

.icon-placeholder {
  background: transparent;
}

.icon-app {
  width: 32px;
  height: 32px;
  border-radius: 6px;
}

.update-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 6px;
  background: oklch(0.55 0.15 250);
  color: oklch(0.98 0 0);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, oklch(1 0 0) 12%, transparent);
  flex-shrink: 0;
}

.update-icon-large {
  width: 44px;
  height: 44px;
  border-radius: 10px;
  align-self: stretch;
}

.update-tile {
  align-items: stretch;
  padding: 12px 14px;
  position: relative;
  overflow: hidden;
}

.update-tile .update-icon-large,
.update-tile .update-body {
  position: relative;
  z-index: 1;
}

.update-tile-progress {
  position: absolute;
  left: 0;
  bottom: 0;
  top: 0;
  background: color-mix(in srgb, var(--accent) 18%, transparent);
  border-right: 1px solid color-mix(in srgb, var(--accent) 55%, transparent);
  transition: width 200ms ease-out;
  pointer-events: none;
  z-index: 0;
}

.update-body {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 4px;
  min-width: 0;
}

.update-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--foreground);
}

.update-description {
  font-size: 12px;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  line-height: 1.4;
}

.update-tile.disabled {
  cursor: default;
}

/* Post-update banner — зелёная галка вместо синей стрелки апдейта.
   Stateless dismiss-on-click, не подсвечивается selection ring'ом. */
.post-update-icon {
  background: oklch(0.62 0.18 145);
}

.post-update-tile {
}

.spin {
  animation: kepler-spin 1s linear infinite;
}

@keyframes kepler-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.icon-builtin {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
  background: linear-gradient(to bottom left, oklch(0.42 0 0), oklch(0.26 0 0));
  color: oklch(0.96 0 0);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, oklch(1 0 0) 6%, transparent);
}

.result:hover {
  background: color-mix(in srgb, oklch(1 0 0) 3.5%, transparent);
}

.result.selected {
  background: color-mix(in srgb, oklch(1 0 0) 8%, transparent);
  border-color: color-mix(in srgb, oklch(1 0 0) 12%, transparent);
}

.title {
  color: var(--foreground);
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 0 1 auto;
  min-width: 0;
}

.subtitle {
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  font-size: 12px;
  flex-shrink: 0;
  margin-left: 12px;
}

.app-name {
  color: color-mix(in srgb, var(--foreground) 42%, transparent);
  font-size: 14px;
  font-weight: 400;
  flex-shrink: 0;
  margin-left: 10px;
}

.kind-label {
  color: color-mix(in srgb, var(--foreground) 32%, transparent);
  font-size: 12px;
  flex-shrink: 0;
  margin-left: auto;
  padding-left: 12px;
}

.empty {
  padding: 32px 22px;
  text-align: center;
  color: color-mix(in srgb, var(--foreground) 40%, transparent);
  font-size: 13px;
}

.actions-backdrop {
  position: absolute;
  inset: 0;
  z-index: 19;
}

.result--hidden {
  opacity: 0.55;
}

.unhide-btn {
  margin-left: auto;
  flex-shrink: 0;
  height: 26px;
  border: 1px solid color-mix(in srgb, var(--foreground) 16%, transparent);
  border-radius: 5px;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
  font: inherit;
  font-size: 12px;
  padding: 0 10px;
  cursor: default;
}

.unhide-btn:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.launcher-menu {
  position: absolute;
  bottom: calc(36px + 8px);
  left: 8px;
  z-index: 20;
  width: 220px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  border-radius: 8px;
  background: var(--surface, #3a3a3e);
  padding: 4px;
  animation: panel-in 120ms cubic-bezier(0.2, 0, 0, 1);
}

.launcher-menu__list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.launcher-menu__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 10px;
  border-radius: 5px;
  cursor: default;
}

.launcher-menu__item:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.launcher-menu__label {
  font-size: 13px;
  color: var(--foreground);
}

.launcher-menu__hint {
  font-size: 13px;
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
}

.launcher-menu__badge {
  min-width: 18px;
  height: 18px;
  border-radius: 9px;
  background: color-mix(in srgb, var(--foreground) 14%, transparent);
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
  font-size: 10px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 5px;
}

.launcher-footer__menu-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 32%, transparent);
  padding: 0;
  cursor: default;
  border-radius: 4px;
}

.launcher-footer__menu-btn:hover {
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
}

@keyframes panel-in {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.launcher-footer__left--mode {
  display: flex;
  align-items: center;
  gap: 7px;
  color: color-mix(in srgb, var(--foreground) 48%, transparent);
  font-size: 12px;
}

.launcher-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 36px;
  flex-shrink: 0;
  padding: 0 14px;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
}

.launcher-footer__left {
  display: flex;
  align-items: center;
  color: color-mix(in srgb, var(--foreground) 32%, transparent);
}

.launcher-footer__actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.launcher-footer__hint-btn {
  display: flex;
  align-items: center;
  gap: 5px;
  border: none;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 52%, transparent);
  font: inherit;
  font-size: 12px;
  cursor: default;
  padding: 0;
  border-radius: 4px;
}

.launcher-footer__hint-btn:hover {
  color: color-mix(in srgb, var(--foreground) 80%, transparent);
}

.launcher-footer__hint-btn--primary {
  color: var(--foreground);
  font-weight: 600;
}

.launcher-footer__sep {
  width: 1px;
  height: 14px;
  background: color-mix(in srgb, var(--foreground) 16%, transparent);
}
</style>
