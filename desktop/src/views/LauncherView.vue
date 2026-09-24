<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted, nextTick, watch } from "vue";
import type { Component } from "vue";
import {
  Settings as SettingsIcon,
  Target as TargetIcon,
  ArrowLeft,
  ArrowUpCircle,
  EyeOff,
  Loader2,
  RefreshCw,
  Check,
  Pause,
  Play,
  SkipForward,
  Square,
  Pencil,
  Calculator,
} from "@lucide/vue";
import { KbdKey, ActionsPanel } from "@kosmos/visuals";
import BuiltInIcon from "../components/BuiltInIcon.vue";
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
import type { CommandRecord, FocusSessionSnapshot, UpdateState } from "@shared/ipc-types";
import { isNumber, isString } from "../shared/runtimeGuards";

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

const BUILTIN_ICONS = {
  "settings:open": {
    icon: SettingsIcon,
    from: "oklch(0.42 0 0)",
    to: "oklch(0.26 0 0)",
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
  "calculator:result": {
    icon: Calculator,
    from: "oklch(0.68 0.15 250)",
    to: "oklch(0.42 0.16 270)",
  },
} satisfies Record<string, BuiltInIconConfig>;

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
  // SAFETY: unknown command ids intentionally have no built-in icon.
  return BUILTIN_ICONS[cmd.id as keyof typeof BUILTIN_ICONS] ?? null;
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
type LauncherMode = "commands" | "focus" | "hidden";
const mode = ref<LauncherMode>("commands");
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
const CALCULATOR_RESULT_ID = "calculator:result";

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

const calculatorResult = ref<{ expression: string; result: string } | null>(null);
const calculatorCommand = computed<CommandRecord | null>(() =>
  calculatorResult.value
    ? {
        id: CALCULATOR_RESULT_ID,
        title: calculatorResult.value.result,
        subtitle: calculatorResult.value.expression,
        category: "action",
        kind: "command",
        appName: "Калькулятор",
      }
    : null,
);

function loadHiddenCommandIds(): string[] {
  try {
    const raw = localStorage.getItem(HIDDEN_COMMANDS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter(isString) : [];
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

const matchedCommands = computed<CommandRecord[]>(() => {
  const q = query.value.trim();
  if (!q) return displayCommands.value;
  return displayCommands.value
    .map<ScoredCommand>((cmd) => ({ cmd, score: scoreCommand(cmd, q) }))
    .filter((x) => x.score >= 0)
    .sort((a, b) => b.score - a.score)
    .map((x) => x.cmd);
});

const filteredCommands = computed<CommandRecord[]>(() =>
  calculatorCommand.value
    ? [calculatorCommand.value, ...matchedCommands.value]
    : matchedCommands.value,
);

const searchPlaceholder = computed(() => "Поиск команд, приложений и файлов");

const headerTitle = computed(() =>
  mode.value === "focus" ? "Фокус" : mode.value === "hidden" ? "Скрытые команды" : "",
);

const RECENTS_KEY = "kepler.launcher.recents";
const RECENTS_LIMIT = 5;

// --- State restore (TTL-bound) ---------------------------------------------
// Сохраняем последние { mode, query, selectedIndex, scrollTop, savedAt } в
// localStorage. При показе launcher'а — если прошло меньше TTL минут,
// восстанавливаем. Иначе сбрасываем. TTL настраивается в Settings →
// kosmos-settings.json::launcherStateTtlMinutes (default 5).
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
    // SAFETY: the surrounding domain validation preserves the asserted contract.
    const parsed = JSON.parse(raw) as PersistedLauncherState;
    if (
      !isString(parsed.query) ||
      !isNumber(parsed.selectedIndex) ||
      !isNumber(parsed.scrollTop) ||
      !isNumber(parsed.savedAt)
    )
      return null;
    if (parsed.mode !== undefined && parsed.mode !== "commands" && parsed.mode !== "focus")
      return null;
    return parsed;
  } catch {
    return null;
  }
}

function savePersistedState() {
  if (!("window" in globalThis)) return;
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
    return Array.isArray(arr) ? arr.filter(isString) : [];
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
    return Array.isArray(arr) ? arr.filter(isString) : [];
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
      title: `Обновить Kosmos до ${s.version}`,
      description: "Установить новую версию и перезапустить",
      icon: ArrowUpCircle,
      spinning: false,
      clickable: true,
    };
  }
  if (s.kind === "downloading") {
    return {
      title: `Скачивается Kosmos ${s.version}`,
      description: `Загружено ${Math.round(s.percent)}%. После завершения можно установить.`,
      icon: Loader2,
      spinning: true,
      clickable: false,
      progress: s.percent,
    };
  }
  if (s.kind === "available") {
    return {
      title: `Доступно обновление Kosmos ${s.version}`,
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
  if (row.cmd.id === CALCULATOR_RESULT_ID) return null;
  return row.cmd;
});
const calculatorSelected = computed(() => {
  const row = rowAt(selectedIndex.value);
  return row?.kind === "cmd" && row.cmd.id === CALCULATOR_RESULT_ID;
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
  if (mode.value !== "commands") return;
  // SAFETY: the surrounding domain validation preserves the asserted contract.
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

function rowAt(idx: number): { kind: "banner" } | { kind: "cmd"; cmd: CommandRecord } | null {
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
  if (row.kind === "banner") {
    await onBannerClick();
    return;
  }
  if (row.cmd.id === CALCULATOR_RESULT_ID && calculatorResult.value) {
    try {
      await navigator.clipboard.writeText(calculatorResult.value.result);
      query.value = "";
      selectedIndex.value = 0;
      await window.kepler.window.hide();
    } catch (error) {
      console.warn("calculator result copy failed", error);
    }
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
  if (mode.value === "focus") {
    leaveFocusMode();
    return;
  }
  if (mode.value === "hidden") {
    leaveHiddenMode();
  }
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
      // SAFETY: the surrounding domain validation preserves the asserted contract.
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
    // SAFETY: the surrounding domain validation preserves the asserted contract.
    return [] as CommandRecord[];
  });
  if (run !== commandsRefreshRun) return; // более свежий refresh победил
  // Фаза 1: built-in команды появляются мгновенно, но уже известные
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
let offFocusOpen = () => {};
let offFocusSessionUpdated = () => {};
let offCommandVisibilityStorage = () => {};
let offBackendReady = () => {};
let offHide = () => {};
let fileSearchTimer: ReturnType<typeof setTimeout> | null = null;
let fileSearchRun = 0;
let calculatorTimer: ReturnType<typeof setTimeout> | null = null;
let calculatorRun = 0;
const FILE_SEARCH_DEBOUNCE_MS = 120;
const MIN_FILE_SEARCH_QUERY_CHARS = 3;
const CALCULATOR_DEBOUNCE_MS = 60;
const MAX_CALCULATOR_QUERY_CHARS = 256;

function cancelCalculator(clearResult = true): void {
  calculatorRun++;
  if (clearResult) calculatorResult.value = null;
  if (calculatorTimer) {
    clearTimeout(calculatorTimer);
    calculatorTimer = null;
  }
}

function looksLikeCalculation(text: string): boolean {
  return (
    text.length <= MAX_CALCULATOR_QUERY_CHARS &&
    /\d/.test(text) &&
    /[\p{L}()+\-*/^%@×÷]/u.test(text)
  );
}

function scheduleCalculator(text: string): void {
  cancelCalculator(false);
  if (!looksLikeCalculation(text)) {
    calculatorResult.value = null;
    return;
  }
  const run = ++calculatorRun;
  calculatorTimer = setTimeout(async () => {
    calculatorTimer = null;
    try {
      const response = await window.kepler.ark.request<{
        result: string | null;
        expression?: string;
      }>("calculator.evaluate", { query: text });
      if (run === calculatorRun) {
        calculatorResult.value = response.result
          ? { expression: response.expression ?? text, result: response.result }
          : null;
      }
    } catch (error) {
      if (run === calculatorRun) calculatorResult.value = null;
      console.warn("calculator.evaluate failed", error);
    }
  }, CALCULATOR_DEBOUNCE_MS);
}

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
    cancelCalculator();
    cancelFileSearch();
    return;
  }
  const text = value.trim();
  scheduleCalculator(text);
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
    } else {
      mode.value = "commands";
      query.value = "";
      selectedIndex.value = 0;
    }
    if (mode.value === "commands") scheduleCalculator(query.value.trim());
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
    cancelCalculator();
    cancelFileSearch();
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
  offFocusSessionUpdated();
  offFocusOpen();
  offCommandVisibilityStorage();
  offBackendReady();
  offHide();
  cancelCalculator();
  cancelFileSearch();
  unsubUpdateState?.();
  unsubPostUpdate?.();
});
</script>

<template src="./LauncherView.html"></template>

<style src="./LauncherView.css" scoped></style>
