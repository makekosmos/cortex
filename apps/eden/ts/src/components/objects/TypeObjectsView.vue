<script setup lang="ts">
import { computed } from "vue";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import {
  formatObjectFieldValue,
  formatReadableRussianDate,
} from "@/lib/objectFieldFormatting";
import {
  getNoteTypeCollectionName,
  parseNoteTypeDefinition,
  parseNoteTypeUiSchema,
  type ResolvedNoteTypeField,
  resolveNoteTypeFields,
} from "@/lib/typedNotes";

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
const iconSrc = computed(() => `/anytype/icon/type/default/${props.noteType.icon || "document"}.svg`);

const collectionEntries = computed(() =>
  props.entries
    .filter((entry) => entry.type_id === props.noteType.id)
    .sort((left, right) => right.updated_at - left.updated_at),
);

const summaryFields = computed(() => {
  const resolved = resolveNoteTypeFields(props.noteType);
  const preferredIds = parseNoteTypeUiSchema(props.noteType.ui_schema_json).featured_fields ?? [];
  const orderedIds = new Map(preferredIds.map((fieldId, index) => [fieldId, index]));
  const definitionFields = parseNoteTypeDefinition(props.noteType.schema_json).fields;
  const definitionOrder = new Map(definitionFields.map((field, index) => [field.id, index]));

  return [...resolved]
    .filter((field) => field.visible)
    .filter((field) => field.kind !== "image" && field.kind !== "long_text")
    .filter((field) => field.id !== "description")
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
    "120px",
  ].join(" "),
}));
</script>

<template>
  <section class="type-objects-view" data-testid="type-objects-view">
    <header class="type-objects-header">
      <div class="type-objects-title-wrap">
        <span
          class="type-objects-icon-wrap"
          :style="{ '--type-accent': noteType.color || '#2aa7ee' }"
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
        <button class="type-objects-secondary-btn" type="button" @click="emit('editType')">
          Р РµРґР°РєС‚РёСЂРѕРІР°С‚СЊ С‚РёРї
        </button>
        <button class="type-objects-primary-btn" type="button" @click="emit('createEntry')">
          РќРѕРІС‹Р№
        </button>
      </div>
    </header>

    <section class="type-objects-panel">
      <div v-if="collectionEntries.length > 0" class="type-objects-table">
        <div class="type-objects-table-head" :style="tableColumnsStyle">
          <span>РќР°Р·РІР°РЅРёРµ</span>
          <span v-for="field in summaryFields" :key="field.id">{{ field.label }}</span>
          <span>РћР±РЅРѕРІР»РµРЅРѕ</span>
        </div>

        <button
          v-for="entry in collectionEntries"
          :key="entry.id"
          class="type-objects-row"
          :style="tableColumnsStyle"
          type="button"
          @click="emit('openEntry', entry.id)"
        >
          <span class="type-objects-name-cell">
            <span
              class="type-objects-row-icon-wrap"
              :style="{ '--type-accent': noteType.color || '#2aa7ee' }"
            >
              <span
                class="type-objects-row-icon"
                :style="{ '--type-icon-src': `url(${iconSrc})` }"
                aria-hidden="true"
              />
            </span>
            <span class="type-objects-name-text">
              {{ getEntryDisplayTitle(entry.title, entry.header_props_json) }}
            </span>
          </span>

          <span v-for="field in summaryFields" :key="field.id">
            {{ formatFieldValue(field, parseHeaderProps(entry)[field.id]) }}
          </span>

          <time>{{ formatDate(entry.updated_at) }}</time>
        </button>
      </div>

      <div v-else class="type-objects-empty">
        <h2>РџРѕРєР° РЅРµС‚ РѕР±СЉРµРєС‚РѕРІ СЌС‚РѕРіРѕ С‚РёРїР°</h2>
        <p>
          РЎРѕР·РґР°Р№ РїРµСЂРІС‹Р№ РѕР±СЉРµРєС‚ С‚РёРїР° В«{{ noteType.name }}В», Рё Р·РґРµСЃСЊ РїРѕСЏРІРёС‚СЃСЏ РїРѕР»РЅРѕС†РµРЅРЅР°СЏ РєРѕР»Р»РµРєС†РёСЏ.
        </p>
        <button class="type-objects-primary-btn" type="button" @click="emit('createEntry')">
          РЎРѕР·РґР°С‚СЊ РѕР±СЉРµРєС‚
        </button>
      </div>
    </section>
  </section>
</template>

<style scoped>
.type-objects-view {
  display: grid;
  gap: 24px;
  min-width: 0;
  min-height: 100%;
  padding: 36px 40px 40px;
  background: var(--background);
}

.type-objects-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
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
}

.type-objects-row-icon-wrap {
  width: 18px;
  height: 18px;
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
  width: 18px;
  height: 18px;
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

.type-objects-primary-btn,
.type-objects-secondary-btn {
  min-height: 38px;
  padding: 0 14px;
  border-radius: 999px;
  font-size: 13px;
  font-weight: 600;
  transition:
    background-color 140ms ease,
    border-color 140ms ease,
    color 140ms ease;
}

.type-objects-primary-btn {
  border: 1px solid color-mix(in srgb, var(--accent) 60%, transparent);
  background: var(--accent);
  color: var(--accent-foreground);
}

.type-objects-primary-btn:hover {
  background: color-mix(in srgb, var(--accent) 88%, black);
}

.type-objects-secondary-btn {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--foreground);
}

.type-objects-secondary-btn:hover {
  background: color-mix(in srgb, var(--foreground) 6%, var(--surface));
}

.type-objects-panel {
  min-width: 0;
  border: 1px solid var(--border);
  border-radius: 22px;
  background: var(--surface);
  overflow: hidden;
}

.type-objects-table {
  display: grid;
}

.type-objects-table-head,
.type-objects-row {
  display: grid;
  gap: 18px;
  align-items: center;
  padding: 14px 18px;
}

.type-objects-table-head {
  border-bottom: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.02em;
}

.type-objects-row {
  border-bottom: 1px solid color-mix(in srgb, var(--border) 76%, transparent);
  background: transparent;
  color: var(--foreground);
  text-align: left;
}

.type-objects-row:last-child {
  border-bottom: none;
}

.type-objects-row:hover {
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
}

.type-objects-name-cell {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.type-objects-name-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.type-objects-empty {
  display: grid;
  gap: 10px;
  justify-items: start;
  padding: 28px;
}

.type-objects-empty h2,
.type-objects-empty p {
  margin: 0;
}

.type-objects-empty p {
  color: var(--muted-foreground);
}

@media (max-width: 920px) {
  .type-objects-view {
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
    padding: 12px 14px;
  }
}
</style>
