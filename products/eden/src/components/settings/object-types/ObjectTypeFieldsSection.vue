<script setup lang="ts">
import { computed, ref } from "vue";
import type { TypeDraft, TypeEditorFieldDraft } from "./shared";
import ObjectTypeFieldRow from "./ObjectTypeFieldRow.vue";

const props = defineProps<{
  draft: TypeDraft;
  isSystemDraft: boolean;
}>();

const emit = defineEmits<{
  addField: [];
  removeField: [fieldId: string];
  moveField: [index: number, direction: -1 | 1];
  moveFieldToIndex: [sourceIndex: number, targetIndex: number];
  updateField: [index: number, patch: Partial<TypeEditorFieldDraft>];
}>();

const dragIndex = ref<number | null>(null);
const dropIndex = ref<number | null>(null);

const canAddField = computed(() => !props.isSystemDraft);
const fieldGroups = computed(() => [
  {
    id: "featured",
    title: "В шапке",
    description: "Главные свойства, которые читаются сразу под типом и описанием.",
    emptyText: "Пока нет свойств для шапки.",
    items: props.draft.fields
      .map((field, index) => ({ field, index }))
      .filter(({ field }) => field.displayMode === "featured"),
  },
  {
    id: "visible",
    title: "В свойствах",
    description: "Обычные свойства, которые отображаются ниже как строки метаданных.",
    emptyText: "Пока нет обычных видимых свойств.",
    items: props.draft.fields
      .map((field, index) => ({ field, index }))
      .filter(({ field }) => field.displayMode === "visible"),
  },
  {
    id: "hidden",
    title: "Скрытые",
    description: "Служебные данные, которые остаются в схеме, но не попадают в обычную шапку.",
    emptyText: "Скрытых свойств пока нет.",
    items: props.draft.fields
      .map((field, index) => ({ field, index }))
      .filter(({ field }) => field.displayMode === "hidden"),
  },
]);

function onDragStart(index: number) {
  dragIndex.value = index;
  dropIndex.value = index;
}

function onDragEnter(index: number) {
  if (dragIndex.value === null) {
    return;
  }

  dropIndex.value = index;
}

function onDragEnd() {
  if (dragIndex.value !== null && dropIndex.value !== null && dragIndex.value !== dropIndex.value) {
    emit("moveFieldToIndex", dragIndex.value, dropIndex.value);
  }

  dragIndex.value = null;
  dropIndex.value = null;
}

function setDisplayMode(index: number, displayMode: TypeEditorFieldDraft["displayMode"]) {
  emit("updateField", index, { displayMode });
}
</script>

<template>
  <section class="object-type-section object-type-section--fields">
    <div class="object-type-section__head">
      <div class="object-type-section__heading">
        <div class="object-type-section__eyebrow">Схема</div>
        <h3 class="object-type-section__title">Слои свойств</h3>
        <p class="object-type-section__text">
          Это прямой аналог Anytype buckets: одни свойства живут в шапке, другие ниже, третьи
          остаются скрытыми. Порядок можно менять drag-and-drop.
        </p>
      </div>

      <button
        class="object-type-fields__add"
        type="button"
        :disabled="!canAddField"
        @click="emit('addField')"
      >
        Добавить поле
      </button>
    </div>

    <div v-if="draft.fields.length === 0" class="object-type-fields__empty">
      Поля еще не добавлены. Сначала задайте структуру типа, затем распределите свойства по слоям.
    </div>

    <div v-else class="object-type-field-buckets">
      <section
        v-for="group in fieldGroups"
        :key="group.id"
        class="object-type-field-bucket"
        :data-testid="`object-type-bucket-${group.id}`"
      >
        <header class="object-type-field-bucket__head">
          <div>
            <div class="object-type-field-bucket__title">{{ group.title }}</div>
            <div class="object-type-field-bucket__text">{{ group.description }}</div>
          </div>
          <div class="object-type-field-bucket__count">{{ group.items.length }}</div>
        </header>

        <div v-if="group.items.length === 0" class="object-type-field-bucket__empty">
          {{ group.emptyText }}
        </div>

        <div v-else class="object-type-field-bucket__items">
          <ObjectTypeFieldRow
            v-for="{ field, index } in group.items"
            :key="field.id"
            :field="field"
            :index="index"
            :is-dragging="dragIndex === index"
            :is-drop-target="dropIndex === index && dragIndex !== index"
            @update-field="(i, patch) => emit('updateField', i, patch)"
            @move-field="(i, dir) => emit('moveField', i, dir)"
            @remove-field="(id) => emit('removeField', id)"
            @set-display-mode="setDisplayMode"
            @drag-start="onDragStart"
            @drag-enter="onDragEnter"
            @drag-end="onDragEnd"
          />
        </div>
      </section>
    </div>
  </section>
</template>

<style scoped>
.object-type-section {
  display: grid;
  gap: 18px;
  padding: 22px 24px;
  border: 1px solid var(--border);
  border-radius: 20px;
  background: var(--surface);
}

.object-type-section__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.object-type-section__heading {
  max-width: 720px;
}

.object-type-section__eyebrow {
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.object-type-section__title {
  margin: 6px 0 0;
  color: var(--foreground);
  font-size: 22px;
  line-height: 1.08;
  font-weight: 600;
  letter-spacing: -0.03em;
}

.object-type-section__text {
  margin: 10px 0 0;
  color: var(--muted-foreground);
  font-size: 14px;
  line-height: 1.6;
}

.object-type-fields__add {
  min-height: 34px;
  padding: 0 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--background);
  color: var(--foreground);
  font-size: 12px;
  font-weight: 600;
}

.object-type-fields__empty,
.object-type-field-bucket__empty {
  padding: 14px 16px;
  border: 1px dashed var(--border);
  border-radius: 14px;
  color: var(--muted-foreground);
  font-size: 13px;
  line-height: 1.5;
}

.object-type-field-buckets {
  display: grid;
  gap: 18px;
}

.object-type-field-bucket {
  display: grid;
  gap: 10px;
}

.object-type-field-bucket__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.object-type-field-bucket__title {
  color: var(--foreground);
  font-size: 16px;
  line-height: 1.25;
  font-weight: 600;
}

.object-type-field-bucket__text {
  margin-top: 4px;
  color: var(--muted-foreground);
  font-size: 13px;
  line-height: 1.55;
}

.object-type-field-bucket__count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 28px;
  height: 28px;
  padding: 0 8px;
  border-radius: 999px;
  background: var(--background);
  border: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 12px;
  font-weight: 600;
}

.object-type-field-bucket__items {
  display: grid;
}

/* Field-row CSS переехал в ObjectTypeFieldRow.vue. */

@media (max-width: 1100px) {
  .object-type-section__head,
  .object-type-field-bucket__head {
    flex-direction: column;
  }
}
</style>
