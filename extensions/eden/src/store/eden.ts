import { defineStore } from "pinia";

import { ref, computed } from "vue";

import { v4 as uuidv4 } from "uuid";

import { normalizeSlug } from "@/lib/typedNotes";

import { createUntitledEntryHeaderProps } from "@/lib/entryTitles";

import {
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

type ActiveScreen = "notes" | "settings" | "object-types" | "type-collection";

// Persistence для last-visited entry id. Юзер reload'ит окно (Ctrl+R в
// dev) и ожидает что вернётся в ту заметку которую читал. Без этого
// hydrateVaultData всегда открывает дефолтную my-space.
const LAST_ENTRY_STORAGE_KEY = "eden:nav:lastEntryId";

function readLastVisitedEntryId(): string | null {
  try {
    return window.localStorage.getItem(LAST_ENTRY_STORAGE_KEY);
  } catch {
    return null;
  }
}

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

const MY_SPACE_TITLE = "Мое пространство";

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

  const noteTypes = ref<NoteType[]>([]);

  const currentEntry = ref<Entry | null>(null);

  const vaultPath = ref<string | null>(null);

  const recentVaultPaths = ref<string[]>([]);

  const isInitializing = ref(true);
  const isHydratingVault = ref(false);

  const activeScreen = ref<ActiveScreen>("notes");

  const activeSpace = ref<SpaceId>("my-space");

  const activeNoteTypeId = ref<string | null>(null);

  const sortMode = ref<SortMode>("updated_at");

  // Non-reactive save coordination state (mutable internal mechanism)

  const latestSaveTimestamps = new Map<string, number>();

  const saveCoordinators: Record<string, EntrySaveCoordinator> = {};

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

    entries.value = entriesData;

    noteTypes.value = mergeNoteTypesWithSystem(noteTypesData);
    pruneTransientSaveState(latestSaveTimestamps, saveCoordinators, entriesData);

    if (currentEntry.value) {
      currentEntry.value =
        entriesData.find((e) => e.id === currentEntry.value!.id) ??
        currentEntry.value;
    }
  }

  async function hydrateVaultData() {
    if (!window.api || !vaultPath.value) return;

    isHydratingVault.value = true;

    try {
      const [entriesData, noteTypesData] = await Promise.all([
        window.api.listEntries(),
        window.api.listNoteTypes(),
      ]);

      entries.value = entriesData;
      noteTypes.value = mergeNoteTypesWithSystem(noteTypesData);
      pruneTransientSaveState(latestSaveTimestamps, saveCoordinators, entriesData);

      // Restore last visited entry (persisted в navigateTo).
      // Если id невалидный / entry удалена — fallback на my-space.
      const lastVisitedId = readLastVisitedEntryId();
      if (lastVisitedId) {
        const lastEntry = entriesData.find(
          (e) => e.id === lastVisitedId && !e.deleted_at,
        );
        if (lastEntry) {
          currentEntry.value = lastEntry;
          activeSpace.value = "diary"; // не открываем my-space welcome
          return;
        }
      }

      const existingMySpace =
        entriesData.find((e) => e.title.trim() === MY_SPACE_TITLE) ?? null;

      if (existingMySpace) {
        currentEntry.value = existingMySpace;
        return;
      }

      const mySpaceEntry: Entry = {
        id: uuidv4(),
        title: MY_SPACE_TITLE,
        content_json: JSON.stringify({
          type: "doc",
          content: [{ type: "paragraph" }],
        }),
        created_at: Date.now(),
        updated_at: Date.now(),
        folder_id: null,
        type_id: SYSTEM_TYPE_NOTE_ID,
        header_layout: null,
        header_props_json: "{}",
        schema_version: 1,
        deleted_at: null,
      };

      entries.value = [mySpaceEntry, ...entries.value];
      void window.api.saveEntry(mySpaceEntry);
      currentEntry.value = mySpaceEntry;
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

    layout.widgetSidebarWidth = sidebarConfig.widget.width;

    layout.widgetSidebarHidden = sidebarConfig.widget.hidden;

    isInitializing.value = false;

    if (path) {
      void hydrateVaultData();
    }
  }

  function createEntry(title: string, noteTypeId: string = SYSTEM_TYPE_NOTE_ID): Entry {
    const newEntry: Entry = {
      id: uuidv4(),

      title,

      content_json: JSON.stringify({
        type: "doc",
        content: [{ type: "paragraph" }],
      }),

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

    if (window.api) {
      void window.api.saveEntry(newEntry).then((result) => {
        if (!result.ok) {
          entries.value = entries.value.filter((e) => e.id !== newEntry.id);

          if (currentEntry.value?.id === newEntry.id) currentEntry.value = null;
        }
      });
    }

    return newEntry;
  }

  function findMySpaceEntry(): Entry | null {
    return entries.value.find((e) => e.title.trim() === MY_SPACE_TITLE) ?? null;
  }

  async function openMySpace() {
    activeScreen.value = "notes";

    activeSpace.value = "my-space";
    activeNoteTypeId.value = null;

    const existing = findMySpaceEntry();

    if (existing) {
      currentEntry.value = existing;

      return;
    }

    currentEntry.value = createEntry(MY_SPACE_TITLE);
  }

  async function createNewEntry(noteTypeId: string = SYSTEM_TYPE_NOTE_ID) {
    activeScreen.value = "notes";

    activeNoteTypeId.value = null;

    const newEntry = createEntry("", noteTypeId);

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
    activeNoteTypeId.value = null;
    // activeSpace = "diary" — иначе watch в App.vue видит default
    // "my-space" && currentEntry=null и openMySpace перебивает нашу
    // загрузку journal entry'и.
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

    // ВСЕГДА перезагружаем entries из ARK перед lookup'ом existing journal.
    // Раньше делали `if (entries.value.length === 0)` — но это ловило только
    // cold-start. В реальном dev-сценарии (HMR App.vue, повторный mount,
    // initApp гонка с user typing) entries.value мог быть устаревшим
    // snapshot'ом без сегодняшнего journal'а → find возвращал null → создавался
    // дубликат пустой записи поверх той, куда пользователь только что писал.
    // listEntries дёшев (in-process WS), стоит лишних ~5ms за надёжность.
    if (window.api) {
      try {
        const fresh = await window.api.listEntries();
        entries.value = fresh;
        console.log("[eden] openTodayJournal: listEntries refreshed,", fresh.length, "entries");
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
    console.log(
      "[eden] openTodayJournal: lookup for",
      todayTitle,
      "→",
      existing ? `found id=${existing.id}` : "not found, will create new",
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
      const idx = entries.value.findIndex((e) => e.id === target.id);
      if (idx >= 0 && fresh) entries.value[idx] = fresh;
      console.log(
        "[eden] openTodayJournal: opening existing journal id=",
        target.id,
        "content length=",
        target.content_json?.length ?? 0,
        "content[:200]=",
        target.content_json?.slice(0, 200) ?? "",
      );
      currentEntry.value = target;
      return;
    }

    // НЕ используем `createEntry` — он жёстко ставит `__untitledTitle: true`
    // в header_props_json, и getEntryDisplayTitle тогда подменяет реальный
    // title "2026-05-19" на placeholder "Без названия". Для дневника title
    // — это и есть смысл, header_props должны быть пустыми (без flag'а).
    const newEntry: Entry = {
      id: uuidv4(),
      title: todayTitle,
      content_json: JSON.stringify({
        type: "doc",
        content: [{ type: "paragraph" }],
      }),
      created_at: Date.now(),
      updated_at: Date.now(),
      folder_id: null,
      type_id: SYSTEM_TYPE_JOURNAL_ID,
      header_layout: null,
      header_props_json: "{}",
      schema_version: 1,
      deleted_at: null,
    };
    console.log("[eden] openTodayJournal: creating new journal id=", newEntry.id);
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

  function openTypeCollection(noteTypeId: string) {
    activeScreen.value = "type-collection";
    activeNoteTypeId.value = noteTypeId;
    activeSpace.value = "all-objects";
    currentEntry.value = null;
  }

  function openObjectTypes(noteTypeId: string | null = null) {
    activeScreen.value = "object-types";
    activeNoteTypeId.value = noteTypeId ?? activeNoteTypeId.value ?? noteTypes.value[0]?.id ?? null;
    currentEntry.value = null;
  }

  async function selectFolder() {
    if (!window.api) return;

    const path = await window.api.selectFolder();

    if (!path) return;

    await window.api.setVaultPath(path);

    vaultPath.value = path;

    recentVaultPaths.value = await window.api.getRecentVaultPaths();

    currentEntry.value = null;

    activeSpace.value = "my-space";
    activeNoteTypeId.value = null;

    await refreshData();
  }

  async function selectVaultPath(nextPath: string) {
    if (!window.api || !nextPath || nextPath === vaultPath.value) return;

    await window.api.setVaultPath(nextPath);

    vaultPath.value = nextPath;

    recentVaultPaths.value = await window.api.getRecentVaultPaths();

    currentEntry.value = null;

    activeSpace.value = "my-space";

    activeScreen.value = "notes";
    activeNoteTypeId.value = null;

    await refreshData();
  }

  async function navigateTo(entryId: string) {
    if (!window.api) return;

    const entry = await window.api.loadEntry(entryId);

    if (entry) {
      activeScreen.value = "notes";
      activeNoteTypeId.value = null;

      currentEntry.value = entry;
      writeLastVisitedEntryId(entryId);
    }
  }

  async function saveNoteType(
    draft: Omit<NoteType, "id" | "created_at" | "updated_at" | "slug"> & {
      id?: string;

      slug?: string;
    },
  ): Promise<SaveNoteTypeResult> {
    if (!window.api)
      return { ok: false, reason: "invalid_definition", message: "No API" };

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

    const persistEntry = async (
      entryToPersist: Entry,
    ): Promise<SaveEntryResult | null> => {
      latestSaveTimestamps.set(entryToPersist.id, entryToPersist.updated_at);

      const result = await window.api.saveEntry(entryToPersist);

      if (!result.ok) return result;

      if (latestSaveTimestamps.get(entryToPersist.id) !== entryToPersist.updated_at)
        return result;

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

    const runSaveLoop = async (
      nextEntry: Entry,
    ): Promise<SaveEntryResult | null> => {
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

  return {
    entries,

    noteTypes,

    currentEntry,

    vaultPath,

    recentVaultPaths,

    isInitializing,

    isHydratingVault,

    activeScreen,

    activeSpace,

    activeNoteTypeId,

    sortMode,

    vaultName,

    refreshData,

    initApp,

    createEntry,

    openMySpace,

    createNewEntry,

    openTodayJournal,

    openTypeCollection,

    openObjectTypes,

    selectFolder,

    selectVaultPath,

    navigateTo,

    saveNoteType,

    deleteNoteType,

    handleSave,
  };
});
