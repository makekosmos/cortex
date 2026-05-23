<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted, nextTick, watch } from "vue";
import type { Component } from "vue";
import {
  Settings as SettingsIcon,
  Database as DatabaseIcon,
  ArrowUpCircle,
  Loader2,
  RefreshCw,
  Check,
} from "@lucide/vue";
import BuiltInIcon from "../components/BuiltInIcon.vue";
import FileSearchResultRow from "../components/FileSearchResultRow.vue";
import holoSvg from "../assets/holo.svg";
import holoPomoSvg from "../assets/holo-pomo.svg";
import holoSecoSvg from "../assets/holo-seco.svg";
import delphiSvg from "../assets/delphi.svg";
import delphiAddSvg from "../assets/delphi-add.svg";
import arraSvg from "../assets/arra.svg";
import edenSvg from "../assets/eden.svg";
import edenAddSvg from "../assets/eden-add.svg";
import edenDiarySvg from "../assets/eden-diary.svg";
import type { CommandRecord, UpdateState } from "@shared/ipc-types";

interface BuiltInIconConfig {
  icon?: Component;
  svgSrc?: string;
  from: string;
  to: string;
  iconColor?: string;
}

// Horologion accent gradient — соответствует --horologion-accent
// (`oklch(0.66 0.245 305)`) из extensions/horologion/src/styles.css.
const HOROLOGION_GRADIENT = {
  from: "oklch(0.66 0.245 305)",
  to: "oklch(0.42 0.20 305)",
};

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
  "horologion:open": { svgSrc: holoSvg, ...HOROLOGION_GRADIENT },
  "horologion:pomodoro:25": { svgSrc: holoPomoSvg, ...HOROLOGION_GRADIENT },
  "horologion:pomodoro:50": { svgSrc: holoPomoSvg, ...HOROLOGION_GRADIENT },
  "horologion:stopwatch:start": { svgSrc: holoSecoSvg, ...HOROLOGION_GRADIENT },
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

function builtInIconFor(cmd: CommandRecord): BuiltInIconConfig | null {
  return BUILTIN_ICONS[cmd.id] ?? null;
}

const query = ref("");
const commands = ref<CommandRecord[]>([]);
const fileCommands = ref<CommandRecord[]>([]);
const selectedIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
const listRef = ref<HTMLDivElement | null>(null);

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
    icon: a.icon_path ?? undefined,
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
  if (!q) return commands.value;
  return commands.value
    .map<ScoredCommand>((cmd) => ({ cmd, score: scoreCommand(cmd, q) }))
    .filter((x) => x.score >= 0)
    .sort((a, b) => b.score - a.score)
    .map((x) => x.cmd);
});

const filtered = computed<CommandRecord[]>(() => {
  if (!query.value.trim()) return commands.value;
  return filteredCommands.value.concat(fileCommands.value);
});

const RECENTS_KEY = "kepler.launcher.recents";
const RECENTS_LIMIT = 5;

// --- State restore (TTL-bound) ---------------------------------------------
// Сохраняем последние { query, selectedIndex, scrollTop, savedAt } в
// localStorage. При показе launcher'а — если прошло меньше TTL минут,
// восстанавливаем. Иначе сбрасываем. TTL настраивается в Settings →
// kepler-shell-settings.json::launcherStateTtlMinutes (default 5).
const STATE_KEY = "kepler.launcher.state";
interface PersistedLauncherState {
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
    return parsed;
  } catch {
    return null;
  }
}

function savePersistedState() {
  if (typeof window === "undefined") return;
  try {
    const state: PersistedLauncherState = {
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
  const byId = new Map(commands.value.map((c) => [c.id, c]));
  const recent: CommandRecord[] = [];
  for (const id of recentIds.value) {
    const c = byId.get(id);
    if (c) recent.push(c);
  }
  // «Все» намеренно содержит все команды — в том числе те, что уже есть в
  // «Недавние». Это дублирование запрошено: список «Все» должен быть полным.
  return { recent, all: commands.value };
});

function onInput() {
  selectedIndex.value = 0;
  scheduleSaveState();
}

function onListScroll() {
  scheduleSaveState();
}

// Шаблон уважает «виртуальный» banner-item впереди: selectedIndex 0 — это
// banner, далее recent, далее all. flatList используется для invocation
// (banner не реальная команда, потому фильтруется).
function totalRows(): number {
  const banner = updateBanner.value ? 1 : 0;
  if (groupedNoQuery.value) {
    return banner + groupedNoQuery.value.recent.length + groupedNoQuery.value.all.length;
  }
  return banner + filteredCommands.value.length + fileCommands.value.length;
}

function rowAt(idx: number): { kind: "banner" } | { kind: "cmd"; cmd: CommandRecord } | null {
  const banner = updateBanner.value ? 1 : 0;
  if (banner && idx === 0) return { kind: "banner" };
  const i = idx - banner;
  if (groupedNoQuery.value) {
    const rec = groupedNoQuery.value.recent;
    const all = groupedNoQuery.value.all;
    if (i < rec.length) return { kind: "cmd", cmd: rec[i]! };
    const j = i - rec.length;
    if (j < all.length) return { kind: "cmd", cmd: all[j]! };
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
  recordRecent(row.cmd.id);
  // App-команда (kind: "app" + id с префиксом `app:`) → app_index.launch.
  // Обычная command → command bus.
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

const SCROLL_EDGE_PADDING = 8;

function moveSelection(delta: number) {
  const n = totalRows();
  if (n === 0) return;
  const prevIdx = selectedIndex.value;
  const next = prevIdx + delta;
  // Clamp без wrap — упереться в границы.
  selectedIndex.value = Math.max(0, Math.min(n - 1, next));
  scheduleSaveState();
  const direction: "up" | "down" = selectedIndex.value < prevIdx ? "up" : "down";
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
  if (e.key === "Escape") {
    e.preventDefault();
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
  }
}

async function refreshCommands() {
  // Загружаем команды от command bus и apps от app_index параллельно,
  // мерджим в один массив. Apps идут после команд (запуск приложения —
  // одна из подкатегорий «open»).
  const [cmds, apps] = await Promise.all([
    window.kepler.commands.list().catch((e) => {
      console.warn("commands.list failed", e);
      return [] as CommandRecord[];
    }),
    fetchApps(),
  ]);
  commands.value = [...cmds, ...apps];
}

let offShow = () => {};
let offCommandsUpdated = () => {};
let fileSearchTimer: ReturnType<typeof setTimeout> | null = null;
let fileSearchRun = 0;
const FILE_SEARCH_DEBOUNCE_MS = 120;
const FILE_SEARCH_REFRESH_MS = 800;

function scheduleFileSearch(text: string, run: number, delay: number) {
  if (fileSearchTimer) clearTimeout(fileSearchTimer);
  fileSearchTimer = setTimeout(async () => {
    const found = await searchFiles(text);
    if (run === fileSearchRun) {
      fileCommands.value = found;
      scheduleFileSearch(text, run, FILE_SEARCH_REFRESH_MS);
    }
  }, delay);
}

watch(query, (value) => {
  const text = value.trim();
  const run = ++fileSearchRun;
  if (!text) {
    if (fileSearchTimer) clearTimeout(fileSearchTimer);
    fileCommands.value = [];
    return;
  }
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
      query.value = persisted.query;
      selectedIndex.value = persisted.selectedIndex;
    } else {
      query.value = "";
      selectedIndex.value = 0;
    }
    void refreshCommands();
    void nextTick(() => {
      inputRef.value?.focus();
      inputRef.value?.select();
      if (listRef.value) {
        listRef.value.scrollTop = fresh && persisted ? persisted.scrollTop : 0;
      }
    });
  });
  offCommandsUpdated = window.kepler.commands.onUpdated(() => {
    void refreshCommands();
  });
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
  fileSearchRun++;
  if (fileSearchTimer) clearTimeout(fileSearchTimer);
  unsubUpdateState?.();
  unsubPostUpdate?.();
});
</script>

<template>
  <div class="launcher" @keydown="onKey">
    <input
      ref="inputRef"
      v-model="query"
      class="search"
      type="text"
      placeholder="Поиск команд, приложений и файлов"
      spellcheck="false"
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      @input="onInput"
    />
    <div ref="listRef" class="list kosmos-scroll" @scroll="onListScroll">
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
        <template v-if="groupedNoQuery.recent.length > 0">
          <div class="section-label">Недавние</div>
          <ul class="results">
            <li
              v-for="(cmd, idx) in groupedNoQuery.recent"
              :key="`recent-${cmd.id}`"
              class="result"
              :class="{ selected: (updateBanner ? 1 : 0) + idx === selectedIndex }"
              @click="
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
              <img v-else-if="cmd.icon" :src="cmd.icon" class="icon" alt="" />
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
                  (updateBanner ? 1 : 0) + groupedNoQuery.recent.length + idx === selectedIndex,
              }"
              @click="
                () => {
                  selectedIndex = (updateBanner ? 1 : 0) + groupedNoQuery!.recent.length + idx;
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
              <img v-else-if="cmd.icon" :src="cmd.icon" class="icon" alt="" />
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
              @click="
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
              <img v-else-if="cmd.icon" :src="cmd.icon" class="icon" alt="" />
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
                :selected="(updateBanner ? 1 : 0) + filteredCommands.length + idx === selectedIndex"
                @select="
                  () => {
                    selectedIndex = (updateBanner ? 1 : 0) + filteredCommands.length + idx;
                    void invokeSelected();
                  }
                "
              />
            </template>
          </ul>
        </template>
      </template>
    </div>
  </div>
</template>

<style scoped>
.launcher {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: color-mix(in srgb, oklch(0.04 0 0) 75%, transparent);
}

.search {
  width: 100%;
  height: 64px;
  padding: 0 22px;
  border: none;
  outline: none;
  background: transparent;
  color: var(--foreground);
  font-size: 18px;
  font-weight: 400;
  flex-shrink: 0;
}

.search::placeholder {
  color: color-mix(in srgb, var(--foreground) 36%, transparent);
}

.list {
  flex: 1;
  overflow-y: auto;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  padding: 8px 0;
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
  cursor: pointer;
  margin: 1px 0;
  /* Без transition: выделение должно срабатывать моментально. */
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
  cursor: pointer;
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
</style>
