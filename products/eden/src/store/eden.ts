import { defineStore } from "pinia";

import { ref, computed, nextTick, watch } from "vue";

import { v4 as uuidv4 } from "uuid";

import {
  SYSTEM_TYPE_COLLECTION_ID,
  SYSTEM_TYPE_JOURNAL,
  SYSTEM_TYPE_JOURNAL_ID,
  SYSTEM_TYPE_NOTE_ID,
} from "@/lib/systemTypes";

import type { SpaceId } from "@/components/sidebar/types";

import type { SortMode } from "@/components/sidebar/types";

import { createBlankEntry, createTodayJournalEntry, todayJournalTitle } from "./edenEntryFactory";
import {
  getCollectionTargetTypeId,
  waitForLoadingFrame,
  type ActiveScreen,
  type EntrySaveCoordinator,
} from "./edenStoreHelpers";
import { createEdenStoreDataActions } from "./edenStoreDataActions";
import { createEdenStoreDraftActions } from "./edenStoreDraftActions";
import { createEdenStoreNoteTypeActions } from "./edenStoreNoteTypeActions";
import { createEdenStoreSaveActions } from "./edenStoreSaveActions";
import { ensureSystemTypePersisted } from "./edenStoreSystemTypeActions";

export const useEdenStore = defineStore("eden", () => {
  const entries = ref<Entry[]>([]);
  const entriesLoaded = ref(false);

  const noteTypes = ref<NoteType[]>([]);

  const currentEntry = ref<Entry | null>(null);

  // Пока впервые догружаем тело заметки через loadEntry(), показываем
  // skeleton по уже известным данным из entries вместо визуального "зависания"
  // на предыдущей/пустой странице.
  const loadingEntryId = ref<string | null>(null);

  // Keep the dirty marker aligned when navigation changes the current entry.
  watch(currentEntry, (entry) => {
    isCurrentEntryDirty.value = !!entry?.id && dirtyEntryId.value === entry.id;
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

  watch(
    activeScreen,
    (screen) => {
      if (screen !== "notes") {
        cancelPendingNavigation();
      }
    },
    { flush: "sync" },
  );

  // Флаг: редактор содержит несохранённые изменения текущей заметки.
  // Выставляется в true когда updateEntryDraft получает черновик с реальными изменениями.
  // Сбрасывается в false когда handleSave успешно завершает.
  // Используется в live-refresh guard: не применять удалённые изменения пока пользователь редактирует.
  const isCurrentEntryDirty = ref(false);
  const dirtyEntryId = ref<string | null>(null);

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

  function createEntry(title: string, noteTypeId: string = SYSTEM_TYPE_NOTE_ID): Entry {
    const newEntry = createBlankEntry({ id: uuidv4(), title, noteTypeId });

    entries.value = [newEntry, ...entries.value];

    return newEntry;
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

    const todayTitle = todayJournalTitle();

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
      if (currentEntry.value?.id === existing.id && isCurrentEntryDirty.value) {
        return;
      }
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
    const newEntry = createTodayJournalEntry(uuidv4(), todayTitle);
    entries.value = [newEntry, ...entries.value];

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
          currentEntry.value = null;
          return;
        }
        currentEntry.value = newEntry;
      } catch (err) {
        console.error("[eden] save journal entry threw:", err);
        entries.value = entries.value.filter((e) => e.id !== newEntry.id);
        currentEntry.value = null;
      }
    }
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

    if (previewEntry?.content_loaded === true) {
      loadingEntryId.value = null;
      currentEntry.value = previewEntry;

      const safeEntry = await ensureEntryCmSafe(previewEntry);
      if (requestSeq !== navigationRequestSeq) return;

      const collectionTypeId = getCollectionTargetTypeId(safeEntry);
      if (collectionTypeId) {
        openTypeCollection(collectionTypeId);
        return;
      }

      upsertEntryBaseline(safeEntry);
      currentEntry.value = safeEntry;
      return;
    }

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

  function openDiary() {
    cancelPendingNavigation();
    activeScreen.value = "diary";
    activeNoteTypeId.value = null;
    activeSpace.value = "diary";
    currentEntry.value = null;
  }

  function openEverything() {
    cancelPendingNavigation();
    activeScreen.value = "notes";
    activeNoteTypeId.value = null;
    activeSpace.value = "diary";
    currentEntry.value = null;
  }

  const { refreshData, initApp, selectFolder, selectVaultPath } = createEdenStoreDataActions({
    activeScreen,
    activeSpace,
    currentEntry,
    entries,
    entriesLoaded,
    isCurrentEntryDirty,
    dirtyEntryId,
    isHydratingVault,
    isInitializing,
    latestSaveTimestamps,
    noteTypes,
    recentVaultPaths,
    saveCoordinators,
    vaultPath,
    ensureEntryCmSafe,
    upsertEntryBaseline,
  });

  const { saveNoteType, deleteNoteType } = createEdenStoreNoteTypeActions({
    noteTypes,
    refreshData,
  });

  const { handleSave } = createEdenStoreSaveActions({
    currentEntry,
    entries,
    isCurrentEntryDirty,
    dirtyEntryId,
    latestSaveTimestamps,
    saveCoordinators,
    markLatestLocalEntry,
  });

  const { updateEntryDraft } = createEdenStoreDraftActions({
    currentEntry,
    entries,
    isCurrentEntryDirty,
    dirtyEntryId,
    noteTypes,
    markLatestLocalEntry,
  });

  return {
    entries,

    noteTypes,

    currentEntry,

    loadingEntryId,

    isCurrentEntryDirty,

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

    openDiary,

    openEverything,

    saveNoteType,

    deleteNoteType,

    handleSave,

    updateEntryDraft,
  };
});
