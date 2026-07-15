<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { Button, Modal, TextInput } from "@kosmos/visuals";
import { normalizeBookLanguage } from "@/lib/bookLanguages";
import {
  BOOK_METADATA_FIELD_IDS,
  extractBookMetadata,
  fillMissingBookMetadata,
  hasExtractedBookData,
  isEmptyBookValue,
  normalizeIsbn,
  type BookMetadata,
  type BookMetadataFieldId,
} from "@/lib/bookMetadata";

const props = defineProps<{
  open: boolean;
  currentTitle: string;
  currentHeaderProps: Record<string, unknown>;
}>();

const emit = defineEmits<{
  close: [];
  apply: [metadata: BookMetadata];
}>();

const source = shallowRef("");
const loading = shallowRef(false);
const error = shallowRef("");
const metadata = shallowRef<BookMetadata | null>(null);
type PreviewFieldId = "title" | BookMetadataFieldId;
const previewFieldIds: PreviewFieldId[] = ["title", ...BOOK_METADATA_FIELD_IDS];
const selectedFields = shallowRef<ReadonlySet<PreviewFieldId>>(new Set());

const sourceIsbn = computed(() => {
  const value = source.value.trim();
  return /^[\dXx\s-]+$/.test(value) ? normalizeIsbn(value) : "";
});
const isValidSourceUrl = computed(() => {
  try {
    const url = new URL(source.value.trim());
    return url.protocol === "https:" && !url.username && !url.password;
  } catch {
    return false;
  }
});
const isValidSource = computed(() => Boolean(sourceIsbn.value) || isValidSourceUrl.value);

const labels: Record<PreviewFieldId, string> = {
  title: "Название",
  author: "Автор",
  cover_image: "Обложка",
  isbn: "ISBN",
  page_count: "Страниц",
  language: "Язык",
  publisher: "Издательство",
  published_date: "Дата издания",
  source_url: "Источник",
};

const previewRows = computed(() => {
  if (!metadata.value) return [];
  return previewFieldIds.flatMap((key) => {
    const value = metadata.value?.[key];
    if (isEmptyBookValue(value)) return [];
    const currentValue = key === "title" ? props.currentTitle : props.currentHeaderProps[key];
    return [
      {
        key,
        label: labels[key],
        value,
        alreadyFilled: !isEmptyBookValue(currentValue),
      },
    ];
  });
});

const selectedCount = computed(
  () => previewRows.value.filter((row) => selectedFields.value.has(row.key)).length,
);

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    source.value = normalizeIsbn(String(props.currentHeaderProps.isbn ?? ""));
    loading.value = false;
    error.value = "";
    metadata.value = null;
    selectedFields.value = new Set();
  },
  { immediate: true },
);

function close(): void {
  if (loading.value) return;
  emit("close");
}

async function loadMetadata(): Promise<void> {
  if (!isValidSource.value || loading.value) return;

  loading.value = true;
  error.value = "";
  metadata.value = null;
  selectedFields.value = new Set();
  try {
    let result: BookMetadata;
    if (sourceIsbn.value) {
      const lookupIsbn = window.kepler?.bookMetadata?.lookupIsbn;
      if (!lookupIsbn) throw new Error("lookup unavailable");
      const found = await lookupIsbn(sourceIsbn.value);
      if (!found) {
        error.value = "Книга с таким ISBN не найдена";
        return;
      }
      result = found;
    } else {
      const fetchPage = window.kepler?.bookMetadata?.fetchPage;
      if (!fetchPage) throw new Error("fetch unavailable");
      const page = await fetchPage(source.value.trim());
      if (!page) throw new Error("empty response");
      const extracted = extractBookMetadata(page);
      result = extracted;
      if (extracted.isbn && window.kepler?.bookMetadata?.lookupIsbn) {
        try {
          const enrichment = await window.kepler.bookMetadata.lookupIsbn(extracted.isbn);
          if (enrichment) result = fillMissingBookMetadata(extracted, enrichment);
        } catch (lookupError) {
          console.warn("[eden:book-metadata] Open Library lookup failed", lookupError);
        }
      }
    }
    if (result.language) result = { ...result, language: normalizeBookLanguage(result.language) };
    if (!hasExtractedBookData(result)) {
      error.value = "На этой странице не удалось распознать данные книги";
      return;
    }
    metadata.value = result;
    selectedFields.value = new Set(
      previewFieldIds.filter((fieldId) => !isEmptyBookValue(result[fieldId])),
    );
  } catch {
    error.value = sourceIsbn.value
      ? "Не удалось получить данные по ISBN. Попробуй ещё раз"
      : "Не удалось загрузить страницу. Проверь ссылку и попробуй ещё раз";
  } finally {
    loading.value = false;
  }
}

function setFieldSelected(fieldId: PreviewFieldId, event: Event): void {
  const next = new Set(selectedFields.value);
  if ((event.target as HTMLInputElement).checked) next.add(fieldId);
  else next.delete(fieldId);
  selectedFields.value = next;
}

function apply(): void {
  if (!metadata.value || selectedCount.value === 0) return;
  const selectedMetadata = Object.fromEntries(
    previewRows.value
      .filter((row) => selectedFields.value.has(row.key))
      .map((row) => [row.key, row.value]),
  ) as BookMetadata;
  emit("apply", selectedMetadata);
  emit("close");
}
</script>

<template>
  <Modal
    :open="open"
    title="Заполнить данные книги"
    width="min(620px, 92vw)"
    hide-close
    @close="close"
  >
    <div class="book-metadata-import">
      <label class="book-metadata-import__label" for="book-metadata-source">
        Ссылка или ISBN
      </label>
      <div class="book-metadata-import__source-row">
        <TextInput
          id="book-metadata-source"
          v-model="source"
          type="text"
          autocomplete="off"
          placeholder="https://www.livelib.ru/book/... или 9780140328721"
          :invalid="Boolean(source.trim()) && !isValidSource"
          :disabled="loading"
          @keydown.enter.prevent="loadMetadata"
        />
        <Button :disabled="!isValidSource || loading" :loading="loading" @click="loadMetadata">
          Найти
        </Button>
      </div>
      <p class="book-metadata-import__hint">
        Выбранные поля заменят текущие. Сними галочку с данных, которые не нужно менять.
      </p>

      <p v-if="source.trim() && !isValidSource" class="book-metadata-import__error" role="alert">
        Вставь публичную HTTPS-ссылку или корректный ISBN
      </p>

      <p v-else-if="error" class="book-metadata-import__error" role="alert">{{ error }}</p>

      <div
        v-if="metadata"
        class="book-metadata-import__preview"
        data-testid="book-metadata-preview"
      >
        <div class="book-metadata-import__preview-heading">Найденные данные</div>
        <div class="book-metadata-import__rows">
          <div
            v-for="row in previewRows"
            :key="row.key"
            class="book-metadata-import__row"
            :class="{
              'is-overwrite': row.alreadyFilled,
              'is-excluded': !selectedFields.has(row.key),
            }"
          >
            <input
              :id="`book-metadata-field-${row.key}`"
              class="book-metadata-import__checkbox"
              type="checkbox"
              :checked="selectedFields.has(row.key)"
              :aria-label="`Применить поле «${row.label}»`"
              @change="setFieldSelected(row.key, $event)"
            />
            <label class="book-metadata-import__row-label" :for="`book-metadata-field-${row.key}`">
              {{ row.label }}
            </label>
            <div class="book-metadata-import__row-value">
              <img
                v-if="row.key === 'cover_image'"
                class="book-metadata-import__cover"
                :src="String(row.value)"
                alt="Найденная обложка"
              />
              <span v-else>{{ row.value }}</span>
            </div>
            <div v-if="row.alreadyFilled" class="book-metadata-import__overwrite">
              заменит текущее
            </div>
          </div>
        </div>
        <p v-if="selectedCount === 0" class="book-metadata-import__hint">
          Выбери хотя бы одно поле.
        </p>
      </div>
    </div>

    <template #footer>
      <Button variant="ghost" :disabled="loading" @click="close">Отмена</Button>
      <Button :disabled="!metadata || selectedCount === 0 || loading" @click="apply">
        Применить
      </Button>
    </template>
  </Modal>
</template>

<style scoped>
.book-metadata-import {
  display: grid;
  gap: 12px;
}

.book-metadata-import__label,
.book-metadata-import__preview-heading {
  color: var(--foreground);
  font-size: 14px;
  font-weight: 600;
}

.book-metadata-import__source-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  align-items: center;
}

.book-metadata-import__hint,
.book-metadata-import__error {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 13px;
  line-height: 1.45;
}

.book-metadata-import__error {
  color: var(--destructive, var(--danger, currentColor));
}

.book-metadata-import__preview {
  display: grid;
  gap: 8px;
  margin-top: 4px;
}

.book-metadata-import__rows {
  display: grid;
  border: 1px solid var(--border);
  border-radius: 10px;
  overflow: hidden;
}

.book-metadata-import__row {
  display: grid;
  grid-template-columns: auto minmax(120px, 0.7fr) minmax(0, 1.3fr) auto;
  gap: 12px;
  align-items: center;
  min-height: 44px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
  color: var(--foreground);
  font-size: 13px;
}

.book-metadata-import__row:last-child {
  border-bottom: 0;
}

.book-metadata-import__row.is-excluded {
  opacity: 0.45;
}

.book-metadata-import__row-label,
.book-metadata-import__overwrite {
  color: var(--muted-foreground);
}

.book-metadata-import__checkbox {
  width: 16px;
  height: 16px;
  accent-color: var(--accent);
}

.book-metadata-import__row-value {
  min-width: 0;
  overflow-wrap: anywhere;
}

.book-metadata-import__overwrite {
  font-size: 12px;
  white-space: nowrap;
}

.book-metadata-import__cover {
  display: block;
  width: 40px;
  max-height: 56px;
  border-radius: 3px;
  object-fit: cover;
}

@media (max-width: 560px) {
  .book-metadata-import__source-row,
  .book-metadata-import__row {
    grid-template-columns: 1fr;
  }

  .book-metadata-import__overwrite {
    white-space: normal;
  }
}
</style>
