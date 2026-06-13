<template>
  <div class="settings-tab">
    <h1 class="settings-tab-title">Общие</h1>
    <p class="settings-tab-subtitle">Основные настройки пространства</p>

    <div class="settings-sections">
      <section class="settings-section">
        <div class="settings-section-header">
          <h2>Редактор</h2>
        </div>
        <div class="settings-section-body">
          <SettingsList>
            <SettingsToggleRow
              title="Проверка орфографии"
              description="Подчёркивает слова с возможными опечатками встроенным проверщиком браузера."
              :model-value="preferences.state.spellcheckEnabled"
              data-testid="eden-spellcheck-toggle"
              @update:model-value="preferences.setSpellcheckEnabled"
            />
          </SettingsList>
        </div>
      </section>

      <section class="settings-section">
        <div class="settings-section-header">
          <h2>Отображаемые типы</h2>
        </div>
        <div class="settings-section-body">
          <SettingsList>
            <SettingsToggleRow
              v-for="noteType in eden.noteTypes"
              :key="noteType.id"
              :title="noteType.name"
              :description="noteType.id"
              :model-value="isObjectTypeVisible(noteType.id)"
              @update:model-value="(value) => setObjectTypeVisible(noteType.id, value)"
            />
          </SettingsList>
          <p class="settings-row-desc-plain">Если включены все типы, Eden загружает все объекты.</p>
        </div>
      </section>

      <section class="settings-section">
        <div class="settings-section-header">
          <h2>Обмен с Markdown</h2>
        </div>
        <div class="settings-section-body">
          <SettingsList>
            <SettingsButtonRow
              title="Экспорт текущего объекта"
              description="Сохраняет паспорт объекта в YAML frontmatter, а текст — обычным Markdown."
              button-label="Экспорт"
              :disabled="!eden.currentEntry || markdownBusy"
              :loading="markdownBusy && markdownOperation === 'export'"
              data-testid="eden-export-markdown"
              @click="exportCurrentEntryMarkdown"
            />
            <SettingsButtonRow
              title="Импорт объекта"
              description="Создаёт или обновляет объект из Markdown-файла с YAML frontmatter."
              button-label="Импорт"
              :disabled="markdownBusy"
              :loading="markdownBusy && markdownOperation === 'import'"
              data-testid="eden-import-markdown"
              @click="importEntryMarkdown"
            />
            <SettingsButtonRow
              title="Импорт Obsidian vault"
              description="Рекурсивно импортирует Markdown-файлы и изображения из выбранной папки."
              button-label="Импорт vault"
              :disabled="markdownBusy"
              :loading="markdownBusy && markdownOperation === 'import-vault'"
              data-testid="eden-import-obsidian-vault"
              @click="importObsidianVaultFolder"
            />
            <SettingsButtonRow
              title="Экспорт всех объектов"
              description="Сохраняет текущие объекты Eden в папку как Obsidian-compatible Markdown."
              button-label="Экспорт vault"
              :disabled="markdownBusy || eden.entries.length === 0"
              :loading="markdownBusy && markdownOperation === 'export-vault'"
              data-testid="eden-export-obsidian-vault"
              @click="exportObsidianVaultFolder"
            />
          </SettingsList>
          <p v-if="markdownStatus" class="settings-row-desc-plain">
            {{ markdownStatus }}
          </p>
          <div v-if="markdownProgress" class="settings-markdown-progress" role="status">
            <div class="settings-markdown-progress-head">
              <span>{{ markdownProgress.label }}</span>
              <span>{{ markdownProgress.current }} / {{ markdownProgress.total }}</span>
            </div>
            <div class="settings-markdown-progress-track">
              <div
                class="settings-markdown-progress-fill"
                :style="{ width: `${markdownProgressPercent}%` }"
              />
            </div>
            <p v-if="markdownProgress.item" class="settings-markdown-progress-item">
              {{ markdownProgress.item }}
            </p>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup vapor lang="ts">
import { computed, onMounted, ref } from "vue";
import { v4 as uuidv4 } from "uuid";
import { SettingsButtonRow, SettingsList, SettingsToggleRow } from "@kosmos/visuals";
import { usePreferences } from "@/composables/usePreferences";
import { buildEntryMarkdownDocument, parseEntryMarkdownDocument } from "@/lib/markdownFrontmatter";
import {
  buildObsidianRelatedImportPlan,
  buildObsidianExportFiles,
  importObsidianVault,
  type ObsidianImportDraft,
} from "@/lib/obsidianVault";
import { SYSTEM_TYPE_IMAGE, SYSTEM_TYPE_IMAGE_ID, SYSTEM_TYPE_NOTE_ID } from "@/lib/systemTypes";
import { normalizeHeaderProps, type NoteType } from "@/lib/typedNotes";
import { useEdenStore } from "@/store/eden";

const preferences = usePreferences();
const eden = useEdenStore();
const markdownBusy = ref(false);
const markdownOperation = ref<"export" | "import" | "import-vault" | "export-vault" | null>(null);
const markdownStatus = ref("");
const markdownProgress = ref<{
  label: string;
  current: number;
  total: number;
  item: string;
} | null>(null);
const visibleObjectTypeIds = ref<string[]>([]);

const noteTypesById = computed(
  () => new Map(eden.noteTypes.map((noteType) => [noteType.id, noteType])),
);
const entryTitlesById = computed(
  () => new Map(eden.entries.map((entry) => [entry.id, entry.title])),
);
const markdownProgressPercent = computed(() => {
  const progress = markdownProgress.value;
  if (!progress || progress.total <= 0) return 0;
  return Math.max(0, Math.min(100, Math.round((progress.current / progress.total) * 100)));
});

const allObjectTypeIds = computed(() => eden.noteTypes.map((noteType) => noteType.id));

onMounted(async () => {
  visibleObjectTypeIds.value = await window.api.getEdenVisibleObjectTypeIds();
});

function visibleObjectTypeSet(): Set<string> {
  return visibleObjectTypeIds.value.length > 0
    ? new Set(visibleObjectTypeIds.value)
    : new Set(allObjectTypeIds.value);
}

function isObjectTypeVisible(typeId: string): boolean {
  return visibleObjectTypeSet().has(typeId);
}

async function setObjectTypeVisible(typeId: string, visible: boolean): Promise<void> {
  const current = visibleObjectTypeSet();
  if (visible) {
    current.add(typeId);
  } else {
    current.delete(typeId);
  }

  const next = allObjectTypeIds.value.filter((id) => current.has(id));
  const normalizedNext = next.length === allObjectTypeIds.value.length ? [] : next;
  visibleObjectTypeIds.value = await window.api.setEdenVisibleObjectTypeIds(normalizedNext);
  await eden.refreshData();
}

function markdownFileName(title: string): string {
  const safeTitle = title
    .trim()
    .replace(/[<>:"/\\|?*]/g, "-")
    .replace(/./g, (char) => (char.charCodeAt(0) < 32 ? "-" : char))
    .replace(/\s+/g, " ");
  return `${safeTitle || "eden-object"}.md`;
}

function resolveImportedRelatedNotes(value: unknown): string[] {
  if (!Array.isArray(value)) return [];

  const byId = new Map(eden.entries.map((entry) => [entry.id, entry.id]));
  const byTitle = new Map(
    eden.entries.map((entry) => [entry.title.trim().toLocaleLowerCase("ru"), entry.id]),
  );

  return value
    .filter((item): item is string => typeof item === "string")
    .map((item) => item.trim())
    .filter(Boolean)
    .map((target) => byId.get(target) ?? byTitle.get(target.toLocaleLowerCase("ru")) ?? null)
    .filter((target): target is string => Boolean(target));
}

async function withMarkdownOperation<T>(
  operation: "export" | "import" | "import-vault" | "export-vault",
  action: () => Promise<T>,
): Promise<T | null> {
  if (markdownBusy.value) return null;
  markdownBusy.value = true;
  markdownOperation.value = operation;
  markdownStatus.value = "";
  markdownProgress.value = null;
  try {
    return await action();
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    markdownStatus.value = `Ошибка: ${message}`;
    return null;
  } finally {
    markdownBusy.value = false;
    markdownOperation.value = null;
    markdownProgress.value = null;
  }
}

async function createMarkdownConverter() {
  const { createMdConverter } = await import("@/editor-cm/mdConvert");
  return createMdConverter();
}

async function exportCurrentEntryMarkdown(): Promise<void> {
  await withMarkdownOperation("export", async () => {
    const entry = eden.currentEntry;
    if (!entry) {
      markdownStatus.value = "Сначала открой объект, который нужно экспортировать.";
      return;
    }

    const converter = await createMarkdownConverter();
    try {
      const bodyMarkdown = converter.jsonToMarkdown(JSON.parse(entry.content_json) as object);
      const noteType = entry.type_id ? noteTypesById.value.get(entry.type_id) : null;
      const document = buildEntryMarkdownDocument({
        entry,
        noteType,
        bodyMarkdown,
        relatedEntryTitleLookup: (entryId) => entryTitlesById.value.get(entryId),
      });
      const result = await window.api.saveMarkdownFile(markdownFileName(entry.title), document);
      markdownStatus.value = result ? `Экспортировано: ${result.path}` : "Экспорт отменён.";
    } finally {
      converter.destroy();
    }
  });
}

async function importEntryMarkdown(): Promise<void> {
  await withMarkdownOperation("import", async () => {
    const picked = await window.api.openMarkdownFile();
    if (!picked) {
      markdownStatus.value = "Импорт отменён.";
      return;
    }

    const parsed = parseEntryMarkdownDocument(picked.content, {
      noteTypes: eden.noteTypes,
    });
    const typeId = parsed.entryPatch.typeId;
    const noteType = typeId ? noteTypesById.value.get(typeId) : null;
    if (!typeId || !noteType) {
      throw new Error("Тип объекта из frontmatter не найден в Eden.");
    }

    const converter = await createMarkdownConverter();
    try {
      const headerProps = { ...parsed.entryPatch.headerProps };
      const relatedNotes = resolveImportedRelatedNotes(headerProps.related_notes);
      if (relatedNotes.length > 0) {
        headerProps.related_notes = relatedNotes;
      } else {
        delete headerProps.related_notes;
      }

      const existingEntry = parsed.entryPatch.id
        ? eden.entries.find((entry) => entry.id === parsed.entryPatch.id)
        : null;
      const now = Date.now();
      const entry: Entry = {
        id: existingEntry?.id ?? parsed.entryPatch.id ?? uuidv4(),
        title: parsed.entryPatch.title,
        content_json: JSON.stringify(converter.markdownToJson(parsed.bodyMarkdown)),
        created_at: existingEntry?.created_at ?? now,
        updated_at: now,
        folder_id: existingEntry?.folder_id ?? null,
        type_id: typeId,
        header_layout: existingEntry?.header_layout ?? null,
        header_props_json: JSON.stringify(normalizeHeaderProps(noteType, headerProps)),
        schema_version: existingEntry?.schema_version ?? 1,
        deleted_at: null,
      };

      const result = await window.api.saveEntry(entry);
      if (!result.ok) {
        throw new Error(result.message ?? "Не удалось сохранить импортированный объект.");
      }

      await eden.refreshData();
      const loaded = await window.api.loadEntry(entry.id);
      eden.currentEntry = loaded ?? entry;
      markdownStatus.value = `Импортировано: ${picked.name}`;
    } finally {
      converter.destroy();
    }
  });
}

function existingEntryForImport(draft: ObsidianImportDraft): Entry | null {
  if (draft.id) {
    return eden.entries.find((entry) => entry.id === draft.id) ?? null;
  }

  const normalizedTitle = draft.title.trim().toLocaleLowerCase("ru");
  return (
    eden.entries.find((entry) => entry.title.trim().toLocaleLowerCase("ru") === normalizedTitle) ??
    null
  );
}

function resolveDraftRelatedNotes(
  draft: ObsidianImportDraft,
  importedTitleIds: Map<string, string>,
  entryId: string,
): string[] {
  const existingByTitle = new Map(
    eden.entries.map((entry) => [entry.title.trim().toLocaleLowerCase("ru"), entry.id]),
  );

  return buildObsidianRelatedImportPlan({
    draft,
    entryId,
    importedTitleIds,
    existingTitleIds: existingByTitle,
  }).secondPassRelatedIds;
}

async function updateMarkdownProgress(
  label: string,
  current: number,
  total: number,
  item: string = "",
): Promise<void> {
  markdownProgress.value = {
    label,
    current,
    total,
    item,
  };
  await Promise.resolve();
}

async function importObsidianVaultFolder(): Promise<void> {
  await withMarkdownOperation("import-vault", async () => {
    const vault = await window.api.openMarkdownVault();
    if (!vault) {
      markdownStatus.value = "Импорт vault отменён.";
      return;
    }

    const converter = await createMarkdownConverter();
    try {
      await updateMarkdownProgress("Подготовка импорта", 0, 1, vault.rootPath);
      await window.api.saveNoteType(SYSTEM_TYPE_IMAGE);
      const imageType = SYSTEM_TYPE_IMAGE;
      const imported = importObsidianVault({
        files: vault.files,
        images: vault.images,
        noteTypes: eden.noteTypes,
        defaultTypeId: SYSTEM_TYPE_NOTE_ID,
        imageTypeId: SYSTEM_TYPE_IMAGE_ID,
      });
      const now = Date.now();
      const importedTitleIds = new Map<string, string>();
      const noteEntries: Array<{
        draft: ObsidianImportDraft;
        existingEntry: Entry | null;
        entry: Entry;
        noteType: NoteType | null;
      }> = [];

      for (const draft of imported.entries) {
        const existingEntry = existingEntryForImport(draft);
        const id = existingEntry?.id ?? draft.id ?? uuidv4();
        importedTitleIds.set(draft.title.trim().toLocaleLowerCase("ru"), id);
      }

      let savedNotes = 0;
      for (let index = 0; index < imported.entries.length; index += 1) {
        const draft = imported.entries[index]!;
        await updateMarkdownProgress(
          "Импорт заметок",
          index,
          imported.entries.length,
          draft.relativePath,
        );
        const existingEntry = existingEntryForImport(draft);
        const id =
          existingEntry?.id ?? importedTitleIds.get(draft.title.trim().toLocaleLowerCase("ru"))!;
        const noteType = noteTypesById.value.get(SYSTEM_TYPE_NOTE_ID);
        const headerProps = {
          ...draft.headerProps,
          source_path: draft.sourcePath,
        };
        const entry: Entry = {
          id,
          title: draft.title,
          content_json: JSON.stringify(converter.markdownToJson(draft.bodyMarkdown)),
          created_at: existingEntry?.created_at ?? now,
          updated_at: now,
          folder_id: existingEntry?.folder_id ?? null,
          type_id: SYSTEM_TYPE_NOTE_ID,
          header_layout: existingEntry?.header_layout ?? null,
          header_props_json: JSON.stringify(normalizeHeaderProps(noteType ?? null, headerProps)),
          schema_version: existingEntry?.schema_version ?? 1,
          deleted_at: null,
        };
        const result = await window.api.saveEntry(entry);
        if (!result.ok) {
          throw new Error(result.message ?? `Не удалось импортировать ${draft.relativePath}.`);
        }
        noteEntries.push({ draft, existingEntry, entry, noteType: noteType ?? null });
        savedNotes += 1;
      }

      let linkedNotes = 0;
      for (let index = 0; index < noteEntries.length; index += 1) {
        const item = noteEntries[index]!;
        await updateMarkdownProgress(
          "Связывание заметок",
          index,
          noteEntries.length,
          item.draft.relativePath,
        );
        const relatedNotes = resolveDraftRelatedNotes(item.draft, importedTitleIds, item.entry.id);
        if (relatedNotes.length === 0) {
          continue;
        }
        const headerProps = {
          ...item.draft.headerProps,
          related_notes: relatedNotes,
          source_path: item.draft.sourcePath,
        };
        const result = await window.api.saveEntry({
          ...item.entry,
          header_props_json: JSON.stringify(normalizeHeaderProps(item.noteType, headerProps)),
          updated_at: now,
        });
        if (!result.ok) {
          throw new Error(
            result.message ?? `Не удалось связать заметки для ${item.draft.relativePath}.`,
          );
        }
        linkedNotes += 1;
      }

      let savedImages = 0;
      for (let index = 0; index < imported.images.length; index += 1) {
        const image = imported.images[index]!;
        await updateMarkdownProgress(
          "Импорт изображений",
          index,
          imported.images.length,
          image.title,
        );
        const entry: Entry = {
          id: image.id,
          title: image.title,
          content_json: JSON.stringify(image.contentJson),
          created_at: now,
          updated_at: now,
          folder_id: null,
          type_id: SYSTEM_TYPE_IMAGE_ID,
          header_layout: null,
          header_props_json: JSON.stringify(normalizeHeaderProps(imageType, image.headerProps)),
          schema_version: 1,
          deleted_at: null,
        };
        const result = await window.api.saveEntry(entry);
        if (!result.ok) {
          throw new Error(result.message ?? `Не удалось импортировать изображение ${image.title}.`);
        }
        savedImages += 1;
      }

      await updateMarkdownProgress("Обновление Eden", 1, 1);
      await eden.refreshData();
      markdownStatus.value = `Импортировано: ${savedNotes} заметок, ${savedImages} изображений. Связи обновлены у ${linkedNotes} заметок.`;
    } finally {
      converter.destroy();
    }
  });
}

async function exportObsidianVaultFolder(): Promise<void> {
  await withMarkdownOperation("export-vault", async () => {
    const converter = await createMarkdownConverter();
    try {
      const bodyMarkdownById = new Map<string, string>();
      for (const entry of eden.entries) {
        try {
          bodyMarkdownById.set(
            entry.id,
            converter.jsonToMarkdown(JSON.parse(entry.content_json) as object),
          );
        } catch {
          bodyMarkdownById.set(entry.id, "");
        }
      }

      const files = buildObsidianExportFiles({
        entries: eden.entries,
        noteTypes: eden.noteTypes,
        bodyMarkdownById: (entry) => bodyMarkdownById.get(entry.id) ?? "",
        relatedEntryTitleLookup: (entryId) => entryTitlesById.value.get(entryId),
      });
      const result = await window.api.exportMarkdownVault(files);
      markdownStatus.value = result
        ? `Экспортировано: ${result.exportedCount} файлов в ${result.outputDir}`
        : "Экспорт vault отменён.";
    } finally {
      converter.destroy();
    }
  });
}
</script>
