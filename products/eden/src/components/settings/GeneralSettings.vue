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
            <SettingsToggleRow
              title="Markdown-редактор (бета)"
              description="CodeMirror 6 с live preview в стиле Obsidian."
              :model-value="preferences.state.cmEditorEnabled"
              data-testid="eden-cm-editor-toggle"
              @update:model-value="preferences.setCmEditorEnabled"
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
          </SettingsList>
          <p v-if="markdownStatus" class="settings-row-desc-plain">
            {{ markdownStatus }}
          </p>
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
import { normalizeHeaderProps } from "@/lib/typedNotes";
import { useEdenStore } from "@/store/eden";

const preferences = usePreferences();
const eden = useEdenStore();
const markdownBusy = ref(false);
const markdownOperation = ref<"export" | "import" | null>(null);
const markdownStatus = ref("");
const visibleObjectTypeIds = ref<string[]>([]);

const noteTypesById = computed(
  () => new Map(eden.noteTypes.map((noteType) => [noteType.id, noteType])),
);
const entryTitlesById = computed(
  () => new Map(eden.entries.map((entry) => [entry.id, entry.title])),
);

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
  operation: "export" | "import",
  action: () => Promise<T>,
): Promise<T | null> {
  if (markdownBusy.value) return null;
  markdownBusy.value = true;
  markdownOperation.value = operation;
  markdownStatus.value = "";
  try {
    return await action();
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    markdownStatus.value = `Ошибка: ${message}`;
    return null;
  } finally {
    markdownBusy.value = false;
    markdownOperation.value = null;
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
</script>
