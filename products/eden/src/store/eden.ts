import { defineStore } from "pinia";

import { ref, computed, nextTick, watch } from "vue";

import { v4 as uuidv4 } from "uuid";

import { normalizeSlug } from "@/lib/typedNotes";

import { createUntitledEntryHeaderProps } from "@/lib/entryTitles";

import { readEntryMarkdown, writeEntryMarkdown } from "@/editor-cm/content";

import {
  SYSTEM_TYPE_COLLECTION_ID,
  SYSTEM_TYPE_JOURNAL,
  SYSTEM_TYPE_JOURNAL_ID,
  SYSTEM_TYPE_NOTE_ID,
  SYSTEM_TYPES,
  isSystemType,
  normalizeSystemNoteType,
} from "@/lib/systemTypes";

import type { SpaceId } from "@/components/sidebar/types";

import type { SortMode } from "@/components/sidebar/types";

import { useLayoutStore } from "./layout";

type ActiveScreen = "notes" | "settings" | "type-collection";

// Persistence для last-visited entry id. Юзер reload'ит окно (Ctrl+R в
// dev) и ожидает что вернётся в ту заметку которую читал.
const LAST_ENTRY_STORAGE_KEY = "eden:nav:lastEntryId";

function writeLastVisitedEntryId(id: string): void {
  try {
    window.localStorage.setItem(LAST_ENTRY_STORAGE_KEY, id);
  } catch {
    // localStorage недоступен — silent.
  }
}

function mergeNoteTypesWithSystem(noteTypesData: NoteType[]) {
  const byId = new Map<string, NoteType>();
  for (const systemType of SYSTEM_TYPES) {
    byId.set(systemType.id, normalizeSystemNoteType(systemType));
  }
  for (const noteType of noteTypesData) {
    byId.set(noteType.id, normalizeSystemNoteType(noteType));
  }
  return [...byId.values()];
}

const SYSTEM_TYPES_BY_ID = new Map(SYSTEM_TYPES.map((noteType) => [noteType.id, noteType]));

function normalizedEntryTypeId(entry: Entry): string {
  return entry.type_id ?? SYSTEM_TYPE_NOTE_ID;
}

function parseEntryHeaderProps(entry: Entry): Record<string, unknown> {
  try {
    const parsed = JSON.parse(entry.header_props_json || "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function getCollectionTargetTypeId(entry: Entry | null | undefined): string | null {
  if (!entry || entry.type_id !== SYSTEM_TYPE_COLLECTION_ID) return null;
  const objectTypeId = parseEntryHeaderProps(entry).object_type_id;
  return typeof objectTypeId === "string" && objectTypeId.trim() ? objectTypeId : null;
}

function mergeEntriesById(entries: Entry[], additions: Entry[]): Entry[] {
  if (additions.length === 0) return entries;
  const byId = new Map(entries.map((entry) => [entry.id, entry] as const));
  for (const entry of additions) {
    byId.set(entry.id, entry);
  }
  return [...byId.values()].sort((left, right) => right.updated_at - left.updated_at);
}

function normalizedEntryHeaderLayout(entry: Entry): string | null {
  return entry.header_layout ?? null;
}

function normalizeHeaderPropsJson(raw: string | null | undefined): string {
  if (!raw?.trim()) return JSON.stringify({});

  try {
    return JSON.stringify(JSON.parse(raw) as Record<string, unknown>);
  } catch {
    return JSON.stringify({});
  }
}

function hasUserVisibleEntryChanges(nextEntry: Entry, previousEntry: Entry): boolean {
  if (nextEntry.title !== previousEntry.title) return true;
  if (normalizedEntryTypeId(nextEntry) !== normalizedEntryTypeId(previousEntry)) return true;
  if (normalizedEntryHeaderLayout(nextEntry) !== normalizedEntryHeaderLayout(previousEntry)) {
    return true;
  }
  if (
    normalizeHeaderPropsJson(nextEntry.header_props_json) !==
    normalizeHeaderPropsJson(previousEntry.header_props_json)
  ) {
    return true;
  }

  return (
    readEntryMarkdown(nextEntry.content_json) !== readEntryMarkdown(previousEntry.content_json)
  );
}

interface QueuedSaveRequest {
  entry: Entry;

  waiters: Array<{
    resolve: (result: SaveEntryResult | null) => void;

    reject: (error: unknown) => void;
  }>;
}

interface EntrySaveCoordinator {
  inFlight: boolean;

  queued: QueuedSaveRequest | null;
}

function waitForLoadingFrame(): Promise<void> {
  return new Promise((resolve) => {
    if (typeof window.requestAnimationFrame === "function") {
      // `requestAnimationFrame()` fires before paint. Use two frame turns and
      // only then queue the macrotask so the loading shell has a real chance to
      // hit the screen before IPC/loadEntry blocks the renderer again.
      window.requestAnimationFrame(() => {
        window.requestAnimationFrame(() => {
          window.setTimeout(resolve, 0);
        });
      });
      return;
    }

    window.setTimeout(resolve, 0);
  });
}

function pruneTransientSaveState(
  latestSaveTimestamps: Map<string, number>,
  saveCoordinators: Record<string, EntrySaveCoordinator>,
  existingEntries: Entry[],
) {
  const validIds = new Set(existingEntries.map((entry) => entry.id));

  for (const entryId of latestSaveTimestamps.keys()) {
    if (!validIds.has(entryId)) {
      latestSaveTimestamps.delete(entryId);
    }
  }

  for (const entryId of Object.keys(saveCoordinators)) {
    if (!validIds.has(entryId) && !saveCoordinators[entryId]?.inFlight) {
      delete saveCoordinators[entryId];
    }
  }
}

export const useEdenStore = defineStore("eden", () => {
  const entries = ref<Entry[]>([]);
  const entriesLoaded = ref(false);

  const noteTypes = ref<NoteType[]>([]);

  const currentEntry = ref<Entry | null>(null);

  // Пока впервые догружаем тело заметки через loadEntry(), показываем
  // skeleton по уже известным данным из entries вместо визуального "зависания"
  // на предыдущей/пустой странице.
  const loadingEntryId = ref<string | null>(null);

  // Persist last-visited entry id ВСЕГДА при изменении currentEntry,
  // не только в navigateTo. openTodayJournal / createEntry / иные code paths
  // тоже ставят currentEntry — без watch'а после refresh'а hydrateVaultData
  // читает старый `lastEntryId` и открывает не ту заметку.
  watch(currentEntry, (entry) => {
    if (entry?.id) writeLastVisitedEntryId(entry.id);
  });

  const vaultPath = ref<string | null>(null);

  const recentVaultPaths = ref<string[]>([]);

  const isInitializing = ref(true);
  const isHydratingVault = ref(false);

  const activeScreen = ref<ActiveScreen>("notes");

  const activeNoteTypeId = ref<string | null>(null);

  let navigationRequestSeq = 0;

  function cancelPendingNavigation(): void {
    navigationRequestSeq += 1;
    loadingEntryId.value = null;
  }

  const activeSpace = ref<SpaceId>("diary");

  const sortMode = ref<SortMode>("updated_at");

  watch(activeScreen, (screen) => {
    if (screen !== "notes") {
      cancelPendingNavigation();
    }
  });

  // Non-reactive save coordination state (mutable internal mechanism)

  // Latest local draft/save timestamp per entry. Optimistic drafts must count
  // here too, otherwise an older in-flight save can overwrite live editor text.
  const latestSaveTimestamps = new Map<string, number>();

  const saveCoordinators: Record<string, EntrySaveCoordinator> = {};

  function upsertEntryBaseline(entry: Entry): void {
    const idx = entries.value.findIndex((candidate) => candidate.id === entry.id);
    if (idx >= 0) {
      entries.value[idx] = entry;
    } else {
      entries.value = [entry, ...entries.value];
    }
  }

  function markLatestLocalEntry(entry: Entry): void {
    const previous = latestSaveTimestamps.get(entry.id);
    if (previous === undefined || previous <= entry.updated_at) {
      latestSaveTimestamps.set(entry.id, entry.updated_at);
    }
  }

  const vaultName = computed(() => {
    if (!vaultPath.value) return null;

    return vaultPath.value.split("/").pop() ?? vaultPath.value;
  });

  async function refreshData() {
    if (!window.api) return;

    const [entriesData, noteTypesData] = await Promise.all([
      window.api.listEntries(),

      window.api.listNoteTypes(),
    ]);

    noteTypes.value = mergeNoteTypesWithSystem(noteTypesData);
    const collectionEntries = await window.api.ensureCollectionObjects(noteTypes.value);

    if (currentEntry.value) {
      const refreshed =
        entriesData.find((e) => e.id === currentEntry.value!.id) ?? currentEntry.value;
      const safeCurrentEntry = await ensureEntryCmSafe({
        ...refreshed,
        content_json: currentEntry.value.content_json,
      });
      currentEntry.value = safeCurrentEntry;
      const idx = entriesData.findIndex((entry) => entry.id === safeCurrentEntry.id);
      if (idx >= 0) entriesData[idx] = safeCurrentEntry;
    }

    const nextEntries = mergeEntriesById(entriesData, collectionEntries);
    entries.value = nextEntries;
    entriesLoaded.value = true;
    pruneTransientSaveState(latestSaveTimestamps, saveCoordinators, nextEntries);
  }

  async function hydrateVaultData() {
    if (!window.api || !vaultPath.value) return;

    isHydratingVault.value = true;

    try {
      const [entriesData, noteTypesData] = await Promise.all([
        window.api.listEntries(),
        window.api.listNoteTypes(),
      ]);

      noteTypes.value = mergeNoteTypesWithSystem(noteTypesData);
      const collectionEntries = await window.api.ensureCollectionObjects(noteTypes.value);
      const nextEntries = mergeEntriesById(entriesData, collectionEntries);
      entries.value = nextEntries;
      entriesLoaded.value = true;
      pruneTransientSaveState(latestSaveTimestamps, saveCoordinators, nextEntries);

      // См. postmortems.md § 2026-06-15. Startup must not mount CodeMirror
      // or load a note body just because an old lastEntryId exists.
      currentEntry.value = null;
    } finally {
      isHydratingVault.value = false;
    }
  }

  async function initApp() {
    if (!window.api) return;

    const [path, recentPaths, sidebarConfig] = await Promise.all([
      window.api.getVaultPath(),

      window.api.getRecentVaultPaths(),

      window.api.getSidebarConfig(),
    ]);

    vaultPath.value = path;

    recentVaultPaths.value = recentPaths;

    const layout = useLayoutStore();

    layout.widgetSidebarHidden = sidebarConfig.widget.hidden;
    layout.widgetSidebarWidth = sidebarConfig.widget.width;

    isInitializing.value = false;

    if (path) {
      void hydrateVaultData();
    }
  }

  function createEntry(title: string, noteTypeId: string = SYSTEM_TYPE_NOTE_ID): Entry {
    const newEntry: Entry = {
      id: uuidv4(),

      title,

      content_json: JSON.stringify(writeEntryMarkdown("")),

      created_at: Date.now(),

      updated_at: Date.now(),

      folder_id: null,

      type_id: noteTypeId,

      header_layout: null,

      header_props_json: JSON.stringify(createUntitledEntryHeaderProps()),

      schema_version: 1,

      deleted_at: null,
    };

    entries.value = [newEntry, ...entries.value];

    return newEntry;
  }

  async function ensureSystemTypePersisted(noteTypeId: string): Promise<void> {
    const systemType = SYSTEM_TYPES_BY_ID.get(noteTypeId);
    if (!systemType || !window.api) return;

    const result = await window.api.saveNoteType(systemType);
    if (!result.ok) {
      console.warn("[eden] persist system type failed:", result);
    }
  }

  // Markdown storage is read tolerantly by CM. Do not rewrite legacy/invalid bodies on open;
  // they become Markdown only through normal editor save.
  async function ensureEntryCmSafe(entry: Entry): Promise<Entry> {
    const nextTypeId = entry.type_id ?? SYSTEM_TYPE_NOTE_ID;
    if (entry.type_id === nextTypeId) {
      return entry;
    }

    const normalized: Entry = {
      ...entry,
      type_id: nextTypeId,
    };

    const idx = entries.value.findIndex((candidate) => candidate.id === entry.id);
    if (idx >= 0) {
      entries.value[idx] = normalized;
    }

    if (window.api) {
      try {
        const result = await window.api.saveEntry(normalized);
        if (!result.ok) {
          console.warn("[eden] normalize entry for CM failed:", result);
        }
      } catch (err) {
        console.warn("[eden] normalize entry for CM threw:", err);
      }
    }

    return normalized;
  }

  async function createNewEntry(noteTypeId: string = SYSTEM_TYPE_NOTE_ID) {
    activeScreen.value = "notes";

    await ensureSystemTypePersisted(noteTypeId);

    const newEntry = createEntry("", noteTypeId);

    // См. postmortems.md § 2026-06-16. Первичное пустое сохранение должно
    // завершиться до mount редактора, иначе оно гоняется с первым autosave.
    if (window.api) {
      try {
        const result = await window.api.saveEntry(newEntry);
        if (!result.ok) {
          console.warn("[eden] save new entry failed:", result);
          entries.value = entries.value.filter((e) => e.id !== newEntry.id);
          return;
        }
      } catch (err) {
        console.error("[eden] save new entry threw:", err);
        entries.value = entries.value.filter((e) => e.id !== newEntry.id);
        return;
      }
    }

    currentEntry.value = newEntry;
  }

  /**
   * Найти или создать дневниковую заметку с datestamp = сегодня.
   * Title формат: ISO `YYYY-MM-DD` (deterministic, locale-independent,
   * сортируемый). Жёсткое условие — title заметки **всегда** равен текущей
   * дате, никакой свободной формы.
   *
   * Используется командой `eden:note:open-today`. В App.vue после открытия
   * выставляется zen mode.
   */
  async function openTodayJournal() {
    activeScreen.value = "notes";
    // activeSpace = "diary" — фиксируем текущий вид на дневник при открытии.
    activeSpace.value = "diary";

    // SYSTEM_TYPE_JOURNAL — client-side system type (определён в systemTypes.ts).
    // Backend не знает о нём, пока Eden явно не сделает upsert_object_type.
    // Без persist'а saveEntry проваливается в ensureEntryTypeAvailable.
    // Idempotent — повторные вызовы перезаписывают то же содержимое.
    if (window.api) {
      try {
        await window.api.saveNoteType(SYSTEM_TYPE_JOURNAL);
      } catch (err) {
        console.warn("[eden] persist journal type failed:", err);
      }
    }

    // ISO date — `2026-05-19`. `padStart(2, "0")` для месяца/дня.
    const now = new Date();
    const yyyy = now.getFullYear();
    const mm = String(now.getMonth() + 1).padStart(2, "0");
    const dd = String(now.getDate()).padStart(2, "0");
    const todayTitle = `${yyyy}-${mm}-${dd}`;

    if (window.api && !entriesLoaded.value) {
      try {
        const fresh = await window.api.listEntries();
        entries.value = fresh;
        entriesLoaded.value = true;
      } catch (err) {
        console.warn("[eden] listEntries before today-journal failed:", err);
      }
    }

    const existing = entries.value.find(
      (entry) =>
        entry.type_id === SYSTEM_TYPE_JOURNAL_ID &&
        entry.title.trim() === todayTitle &&
        entry.deleted_at === null,
    );
    if (existing) {
      // Fresh state из ARK — entries.value может быть устаревший snapshot
      // (autosave в Editor.vue обновляет entries[idx] post-persist, но
      // если пользователь вызывает open-today до того как autosave успел —
      // existing.content_json показывает версию ДО его правок). loadEntry
      // даёт source-of-truth, чтобы Editor.vue гидратировался от ARK,
      // а не от stale in-memory.
      const fresh = window.api ? await window.api.loadEntry(existing.id) : null;
      const target = fresh ?? existing;
      if (fresh) upsertEntryBaseline(fresh);
      currentEntry.value = await ensureEntryCmSafe(target);
      return;
    }

    // НЕ используем `createEntry` — он жёстко ставит `__untitledTitle: true`
    // в header_props_json, и getEntryDisplayTitle тогда подменяет реальный
    // title "2026-05-19" на placeholder "Без названия". Для дневника title
    // — это и есть смысл, header_props должны быть пустыми (без flag'а).
    const newEntry: Entry = {
      id: uuidv4(),
      title: todayTitle,
      content_json: JSON.stringify(writeEntryMarkdown("")),
      created_at: Date.now(),
      updated_at: Date.now(),
      folder_id: null,
      type_id: SYSTEM_TYPE_JOURNAL_ID,
      header_layout: null,
      header_props_json: "{}",
      schema_version: 1,
      deleted_at: null,
    };
    entries.value = [newEntry, ...entries.value];
    currentEntry.value = newEntry;

    // Persist первичную пустую заметку СИНХРОННО до того как редактор начнёт
    // autosave'ить пользовательский ввод. Раньше был fire-and-forget
    // `void saveEntry(newEntry)` — он гонялся с Editor.vue handleSave,
    // которая использует другой saveCoordinator (в shim'е). Если empty-save
    // приземлялся в ARK после первого autosave с контентом, ARK не мог
    // надёжно решить кто новее (HLC vs user-provided updated_at) — мог
    // случиться overwrite контента пустой версией. Await гарантирует
    // строгий порядок: создание → запись → редактирование.
    if (window.api) {
      try {
        const result = await window.api.saveEntry(newEntry);
        if (!result.ok) {
          console.warn("[eden] save journal entry failed:", result);
          entries.value = entries.value.filter((e) => e.id !== newEntry.id);
          if (currentEntry.value?.id === newEntry.id) {
            currentEntry.value = null;
          }
        }
      } catch (err) {
        console.error("[eden] save journal entry threw:", err);
      }
    }
  }

  async function selectFolder() {
    if (!window.api) return;

    const path = await window.api.selectFolder();

    if (!path) return;

    await window.api.setVaultPath(path);

    vaultPath.value = path;

    recentVaultPaths.value = await window.api.getRecentVaultPaths();

    currentEntry.value = null;
    entriesLoaded.value = false;

    activeSpace.value = "diary";

    await refreshData();
  }

  async function selectVaultPath(nextPath: string) {
    if (!window.api || !nextPath || nextPath === vaultPath.value) return;

    await window.api.setVaultPath(nextPath);

    vaultPath.value = nextPath;

    recentVaultPaths.value = await window.api.getRecentVaultPaths();

    currentEntry.value = null;
    entriesLoaded.value = false;

    activeSpace.value = "diary";

    activeScreen.value = "notes";

    await refreshData();
  }

  async function navigateTo(entryId: string) {
    if (!window.api) return;

    const requestSeq = ++navigationRequestSeq;
    const previewEntry = entries.value.find((entry) => entry.id === entryId) ?? null;
    const previewCollectionTypeId = getCollectionTargetTypeId(previewEntry);
    if (previewCollectionTypeId) {
      openTypeCollection(previewCollectionTypeId);
      return;
    }

    activeScreen.value = "notes";
    loadingEntryId.value = entryId;
    currentEntry.value = previewEntry;

    try {
      // Важно: дать Vue и браузеру реально нарисовать skeleton до IPC/loadEntry.
      // Иначе синхронная часть bridge/IPC может заблокировать поток, и визуально
      // страница откроется только через ~1s вместе с уже загруженной заметкой.
      await nextTick();
      await waitForLoadingFrame();

      if (requestSeq !== navigationRequestSeq) return;

      const entry = await window.api.loadEntry(entryId);

      // Пользователь мог быстро кликнуть другую заметку, пока эта грузилась,
      // или уйти в settings, что инвалидирует pending navigation.
      if (requestSeq !== navigationRequestSeq) return;

      if (entry) {
        const safeEntry = await ensureEntryCmSafe(entry);
        const collectionTypeId = getCollectionTargetTypeId(safeEntry);
        if (collectionTypeId) {
          openTypeCollection(collectionTypeId);
          return;
        }
        upsertEntryBaseline(safeEntry);
        currentEntry.value = safeEntry;
        writeLastVisitedEntryId(entryId);
      } else {
        currentEntry.value = null;
      }
    } catch (err) {
      console.warn("[eden] loadEntry failed:", err);
    } finally {
      if (requestSeq === navigationRequestSeq) {
        loadingEntryId.value = null;
      }
    }
  }

  function openTypeCollection(noteTypeId: string) {
    cancelPendingNavigation();
    activeScreen.value = "type-collection";
    activeNoteTypeId.value = noteTypeId;
    activeSpace.value = "diary";
    currentEntry.value =
      entries.value.find(
        (entry) =>
          entry.type_id === SYSTEM_TYPE_COLLECTION_ID &&
          getCollectionTargetTypeId(entry) === noteTypeId,
      ) ?? null;
  }

  async function saveNoteType(
    draft: Omit<NoteType, "id" | "created_at" | "updated_at" | "slug"> & {
      id?: string;

      slug?: string;
    },
  ): Promise<SaveNoteTypeResult> {
    if (!window.api) return { ok: false, reason: "invalid_definition", message: "No API" };

    const now = Date.now();

    const noteType: NoteType = {
      id: draft.id ?? uuidv4(),

      name: draft.name,

      slug: normalizeSlug(draft.slug || draft.name),

      icon: draft.icon,

      color: draft.color,

      schema_json: draft.schema_json,

      header_template_json: draft.header_template_json,

      ui_schema_json: draft.ui_schema_json,

      created_at: draft.id
        ? (noteTypes.value.find((t) => t.id === draft.id)?.created_at ?? now)
        : now,

      updated_at: now,
    };

    const result = await window.api.saveNoteType(noteType);

    if (result.ok) await refreshData();

    return result;
  }

  async function deleteNoteType(noteTypeId: string) {
    if (!window.api || isSystemType(noteTypeId)) return;

    await window.api.deleteNoteType(noteTypeId);

    await refreshData();
  }

  async function handleSave(entry: Entry): Promise<SaveEntryResult | null> {
    if (!window.api) return null;

    // См. postmortems.md § 2026-06-15. `entries` contains optimistic drafts
    // from updateEntryDraft, so it is not a safe persisted baseline for skipping saves.
    const persistEntry = async (entryToPersist: Entry): Promise<SaveEntryResult | null> => {
      markLatestLocalEntry(entryToPersist);

      const result = await window.api.saveEntry(entryToPersist);

      if (!result.ok) return result;

      if (latestSaveTimestamps.get(entryToPersist.id) !== entryToPersist.updated_at) return result;

      const idx = entries.value.findIndex((e) => e.id === entryToPersist.id);

      if (idx >= 0) {
        entries.value[idx] = entryToPersist;
      } else {
        entries.value = [entryToPersist, ...entries.value];
      }

      if (currentEntry.value?.id === entryToPersist.id) {
        currentEntry.value = entryToPersist;
      }

      return result;
    };

    const coordinator = saveCoordinators[entry.id] ?? {
      inFlight: false,
      queued: null,
    };

    saveCoordinators[entry.id] = coordinator;

    const runSaveLoop = async (nextEntry: Entry): Promise<SaveEntryResult | null> => {
      coordinator.inFlight = true;

      try {
        const result = await persistEntry(nextEntry);

        const queued = coordinator.queued;

        if (!queued) {
          coordinator.inFlight = false;

          return result;
        }

        coordinator.queued = null;

        const queuedResult = await runSaveLoop(queued.entry);

        queued.waiters.forEach((w) => w.resolve(queuedResult));

        return queuedResult;
      } catch (error) {
        const queued = coordinator.queued;

        coordinator.queued = null;

        coordinator.inFlight = false;

        if (queued) queued.waiters.forEach((w) => w.reject(error));

        throw error;
      } finally {
        if (!coordinator.queued) {
          coordinator.inFlight = false;

          delete saveCoordinators[nextEntry.id];

          if (latestSaveTimestamps.get(nextEntry.id) === nextEntry.updated_at) {
            latestSaveTimestamps.delete(nextEntry.id);
          }
        }
      }
    };

    if (!coordinator.inFlight) return runSaveLoop(entry);

    return new Promise<SaveEntryResult | null>((resolve, reject) => {
      if (coordinator.queued) {
        coordinator.queued.entry = entry;

        coordinator.queued.waiters.push({ resolve, reject });

        return;
      }

      coordinator.queued = { entry, waiters: [{ resolve, reject }] };
    });
  }

  function updateEntryDraft(entry: Entry) {
    // См. postmortems.md § 2026-06-16. Live editor draft is newer than any
    // older in-flight save completion until the same draft is persisted.
    markLatestLocalEntry(entry);

    const idx = entries.value.findIndex((candidate) => candidate.id === entry.id);

    if (idx >= 0) {
      if (!hasUserVisibleEntryChanges(entry, entries.value[idx])) return;
      entries.value[idx] = entry;
    } else {
      entries.value = [entry, ...entries.value];
    }

    if (currentEntry.value?.id === entry.id) {
      currentEntry.value = entry;
    }
  }

  return {
    entries,

    noteTypes,

    currentEntry,

    loadingEntryId,

    vaultPath,

    recentVaultPaths,

    isInitializing,

    isHydratingVault,

    activeScreen,

    activeNoteTypeId,

    activeSpace,

    sortMode,

    vaultName,

    refreshData,

    initApp,

    createEntry,

    createNewEntry,

    openTodayJournal,

    selectFolder,

    selectVaultPath,

    navigateTo,

    openTypeCollection,

    saveNoteType,

    deleteNoteType,

    handleSave,

    updateEntryDraft,
  };
});
