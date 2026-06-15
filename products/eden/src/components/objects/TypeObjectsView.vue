<script setup lang="ts">
import { computed } from "vue";
import { Button, EmptyState } from "@kosmos/visuals";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { formatObjectFieldValue, formatReadableRussianDate } from "@/lib/objectFieldFormatting";
import {
  getNoteTypeCollectionName,
  getNoteTypePresentation,
  parseNoteTypeDefinition,
  parseNoteTypeUiSchema,
  type ResolvedNoteTypeField,
  resolveNoteTypeFields,
} from "@/lib/typedNotes";
import { objectIconUri } from "@/lib/iconResolver";
import { resolveObjectImageSrc } from "@/lib/objectImages";
import { SYSTEM_TYPE_PERSON_ID } from "@/lib/systemTypes";

const props = defineProps<{
  noteType: NoteType;
  entries: Entry[];
}>();

const emit = defineEmits<{
  openEntry: [entryId: string];
  createEntry: [];
  editType: [];
}>();

function parseHeaderProps(entry: Entry): Record<string, unknown> {
  try {
    const parsed = JSON.parse(entry.header_props_json || "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function formatDate(timestamp: number): string {
  return formatReadableRussianDate(timestamp);
}

function formatFieldValue(field: ResolvedNoteTypeField, value: unknown): string {
  return formatObjectFieldValue(field, value) || "\u2014";
}

const collectionTitle = computed(() => getNoteTypeCollectionName(props.noteType));
const iconSrc = computed(() => objectIconUri(props.noteType.icon));
const presentation = computed(() => getNoteTypePresentation(props.noteType));
const isPersonCollection = computed(() => props.noteType.id === SYSTEM_TYPE_PERSON_ID);
const entriesById = computed(
  () => new Map(props.entries.map((entry) => [entry.id, entry] satisfies [string, Entry])),
);

function getEntryImageSrc(entry: Entry): string {
  const imageFieldId = presentation.value.imageFieldId;
  if (!imageFieldId) return "";

  return resolveObjectImageSrc(parseHeaderProps(entry)[imageFieldId], entriesById.value);
}

function getPersonDisplayName(entry: Entry): string {
  const props = parseHeaderProps(entry);
  const displayName = [
    String(props.first_name ?? "").trim(),
    String(props.patronymic ?? "").trim(),
    String(props.last_name ?? "").trim(),
  ]
    .filter(Boolean)
    .join(" ");

  return displayName || getEntryDisplayTitle(entry.title, entry.header_props_json);
}

const collectionEntries = computed(() =>
  props.entries
    .filter((entry) => entry.type_id === props.noteType.id)
    .sort((left, right) => right.updated_at - left.updated_at),
);

const summaryFields = computed(() => {
  const resolved = resolveNoteTypeFields(props.noteType);
  const preferredIds = parseNoteTypeUiSchema(props.noteType.ui_schema_json).featured_fields ?? [];
  const hiddenIds = new Set([
    "description",
    "related_notes",
    "created_at",
    "updated_at",
    "deleted_at",
    "source_path",
  ]);
  const orderedIds = new Map(preferredIds.map((fieldId, index) => [fieldId, index]));
  const definitionFields = parseNoteTypeDefinition(props.noteType.schema_json).fields;
  const definitionOrder = new Map(definitionFields.map((field, index) => [field.id, index]));

  return [...resolved]
    .filter((field) => field.visible)
    .filter((field) => field.kind !== "image" && field.kind !== "long_text")
    .filter((field) => field.kind !== "relation")
    .filter((field) => !hiddenIds.has(field.id))
    .sort((left, right) => {
      const preferredDelta = (orderedIds.get(left.id) ?? 999) - (orderedIds.get(right.id) ?? 999);
      if (preferredDelta !== 0) {
        return preferredDelta;
      }
      return (definitionOrder.get(left.id) ?? 999) - (definitionOrder.get(right.id) ?? 999);
    })
    .slice(0, 2);
});

const tableColumnsStyle = computed(() => ({
  gridTemplateColumns: [
    "minmax(220px, 1.6fr)",
    ...summaryFields.value.map(() => "minmax(140px, 1fr)"),
    "minmax(150px, 0.72fr)",
  ].join(" "),
}));
</script>

<template>
  <section class="type-objects-view" data-testid="type-objects-view">
    <header class="type-objects-header">
      <div class="type-objects-title-wrap">
        <span
          class="type-objects-icon-wrap"
          :style="{ '--type-accent': noteType.color || 'var(--accent)' }"
        >
          <span
            class="type-objects-icon"
            :style="{ '--type-icon-src': `url(${iconSrc})` }"
            aria-hidden="true"
          />
        </span>

        <div class="type-objects-title-copy">
          <h1 class="type-objects-title">{{ collectionTitle }}</h1>
        </div>
      </div>

      <div class="type-objects-actions">
        <Button variant="ghost" size="sm" type="button" @click="emit('editType')">
          Редактировать тип
        </Button>
        <Button size="sm" type="button" @click="emit('createEntry')">Новый</Button>
      </div>
    </header>

    <section class="type-objects-panel">
      <div v-if="collectionEntries.length > 0" class="type-objects-table">
        <div class="type-objects-table-head" :style="tableColumnsStyle">
          <span class="type-objects-table-head-name">
            <span class="type-objects-table-head-icon" aria-hidden="true"></span>
            <span>Название</span>
          </span>
          <span v-for="field in summaryFields" :key="field.id">{{ field.label }}</span>
          <span>Обновлено</span>
        </div>

        <div class="type-objects-table-body kosmos-scroll">
          <div
            v-for="entry in collectionEntries"
            :key="entry.id"
            class="type-objects-row"
            :class="isPersonCollection && 'type-objects-row--person'"
            :style="tableColumnsStyle"
            tabindex="0"
            role="button"
            :data-testid="`type-object-row-${entry.id}`"
            @click="emit('openEntry', entry.id)"
            @keydown.enter.prevent="emit('openEntry', entry.id)"
            @keydown.space.prevent="emit('openEntry', entry.id)"
          >
            <span class="type-objects-name-cell">
              <span
                class="type-objects-row-icon-wrap"
                :style="{ '--type-accent': noteType.color || 'var(--accent)' }"
              >
                <img
                  v-if="getEntryImageSrc(entry)"
                  class="type-objects-row-thumb"
                  :class="isPersonCollection && 'type-objects-row-thumb--avatar'"
                  :src="getEntryImageSrc(entry)"
                  :alt="getEntryDisplayTitle(entry.title, entry.header_props_json)"
                  draggable="false"
                />
                <span
                  v-else
                  class="type-objects-row-icon"
                  :style="{ '--type-icon-src': `url(${iconSrc})` }"
                  aria-hidden="true"
                />
              </span>
              <span class="type-objects-name-text">
                {{
                  isPersonCollection
                    ? getPersonDisplayName(entry)
                    : getEntryDisplayTitle(entry.title, entry.header_props_json)
                }}
              </span>
            </span>

            <span v-for="field in summaryFields" :key="field.id" class="type-objects-cell">
              {{ formatFieldValue(field, parseHeaderProps(entry)[field.id]) }}
            </span>

            <time>{{ formatDate(entry.updated_at) }}</time>
          </div>
        </div>
      </div>

      <EmptyState
        v-else
        data-testid="type-objects-empty"
        title="Пока нет объектов этого типа"
        :description="`Создай первый объект типа «${noteType.name}», и здесь появится полноценная коллекция.`"
      >
        <template #action>
          <Button size="sm" type="button" @click="emit('createEntry')">Создать объект</Button>
        </template>
      </EmptyState>
    </section>
  </section>
</template>

<style scoped>
.type-objects-view {
  display: flex;
  height: 100%;
  min-height: 0;
  min-width: 0;
  flex-direction: column;
  gap: 24px;
  padding: 36px 40px 40px;
  overflow: auto;
  overscroll-behavior: contain;
}

.type-objects-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  flex: 0 0 auto;
}

.type-objects-title-wrap {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 0;
}

.type-objects-icon-wrap,
.type-objects-row-icon-wrap {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.type-objects-icon-wrap {
  width: 28px;
  height: 28px;
  flex-shrink: 0;
}

.type-objects-row-icon-wrap {
  width: 20px;
  height: 20px;
  justify-self: center;
  overflow: hidden;
  border-radius: 4px;
}

.type-objects-icon,
.type-objects-row-icon {
  display: inline-block;
  background-color: var(--type-accent);
  mask-image: var(--type-icon-src);
  mask-repeat: no-repeat;
  mask-position: center;
  mask-size: contain;
  -webkit-mask-image: var(--type-icon-src);
  -webkit-mask-repeat: no-repeat;
  -webkit-mask-position: center;
  -webkit-mask-size: contain;
}

.type-objects-icon {
  width: 24px;
  height: 24px;
}

.type-objects-row-icon {
  width: 20px;
  height: 20px;
}

.type-objects-row-thumb {
  display: block;
  width: 20px;
  height: 20px;
  object-fit: cover;
}

.type-objects-row-thumb--avatar {
  border-radius: 999px;
}

.type-objects-title-copy {
  display: grid;
  min-width: 0;
}

.type-objects-title {
  margin: 0;
  color: var(--foreground);
  font-size: var(--kosmos-text-page-title-size);
  line-height: var(--kosmos-text-page-title-line-height);
  letter-spacing: var(--kosmos-text-page-title-letter-spacing);
  font-weight: var(--kosmos-text-page-title-weight);
}

.type-objects-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.type-objects-panel {
  flex: 0 0 auto;
  min-width: 0;
  min-height: 0;
  overflow: visible;
}

.type-objects-table {
  display: flex;
  width: 100%;
  min-height: 0;
  flex-direction: column;
  overflow: visible;
}

.type-objects-table-head,
.type-objects-row {
  display: grid;
  gap: 16px;
  align-items: center;
}

.type-objects-table-head {
  padding: 8px 20px;
  border-bottom: 1px solid var(--border-color-strong);
  background: var(--main-background-color);
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.5px;
  text-transform: uppercase;
}

.type-objects-table-head-name {
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.type-objects-table-head-icon {
  width: 20px;
  justify-self: center;
}

.type-objects-row {
  min-height: 36px;
  padding: 4px 20px;
  border-bottom: 1px solid color-mix(in srgb, var(--border-color-strong) 72%, transparent);
  color: var(--foreground);
  font-size: 13px;
  cursor: default;
}

.type-objects-row:last-child {
  border-bottom: none;
}

.type-objects-row:hover {
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
}

.type-objects-row:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--accent) 65%, transparent);
  outline-offset: -2px;
}

.type-objects-table-body {
  flex: 0 0 auto;
  min-height: 0;
  overflow: visible;
}

.type-objects-name-cell {
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.type-objects-row--person .type-objects-name-cell {
  align-items: center;
  text-align: center;
}

.type-objects-row--person .type-objects-name-text {
  text-align: center;
}

.type-objects-name-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
}

.type-objects-cell,
.type-objects-row time {
  min-width: 0;
  overflow: hidden;
  color: color-mix(in srgb, var(--foreground) 68%, transparent);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 920px) {
  .type-objects-view {
    height: 100%;
    padding: 28px 24px 32px;
  }

  .type-objects-header {
    flex-direction: column;
    align-items: stretch;
  }

  .type-objects-actions {
    justify-content: flex-start;
  }

  .type-objects-table-head,
  .type-objects-row {
    gap: 12px;
    padding-inline: 14px;
  }
}
</style>
