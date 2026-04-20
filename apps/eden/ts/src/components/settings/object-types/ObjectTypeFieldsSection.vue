<script setup lang="ts">
import { computed, ref } from "vue";
import type { TypeDraft, TypeEditorFieldDraft } from "./shared";
import { FIELD_KIND_OPTIONS } from "./shared";

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
const fieldKindLabels = new Map(FIELD_KIND_OPTIONS.map((option) => [option.value, option.label]));
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
  if (
    dragIndex.value !== null &&
    dropIndex.value !== null &&
    dragIndex.value !== dropIndex.value
  ) {
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
          <article
            v-for="{ field, index } in group.items"
            :key="field.id"
            class="object-type-field-row"
            :class="{
              'is-dragging': dragIndex === index,
              'is-drop-target': dropIndex === index && dragIndex !== index,
            }"
            :draggable="!field.structuralLocked"
            @dragstart="onDragStart(index)"
            @dragenter.prevent="onDragEnter(index)"
            @dragover.prevent
            @dragend="onDragEnd"
            @drop.prevent="onDragEnd"
          >
            <div class="object-type-field-row__overlay" aria-hidden="true" />

            <div class="object-type-field-row__main">
              <button
                class="object-type-field-row__drag"
                type="button"
                :disabled="field.structuralLocked"
                title="Перетащить поле"
                @mousedown.prevent
              >
                <span />
                <span />
                <span />
                <span />
                <span />
                <span />
              </button>

              <div class="object-type-field-row__content">
                <div class="object-type-field-row__topline">
                  <div class="object-type-field-row__kind">{{ fieldKindLabels.get(field.kind) }}</div>
                  <div class="object-type-field-row__id">{{ field.id }}</div>
                </div>

                <div class="object-type-field-row__identity">
                  <input
                    class="object-type-form-input object-type-form-input--title"
                    :value="field.label"
                    :disabled="field.structuralLocked"
                    placeholder="Название поля"
                    @input="emit('updateField', index, { label: ($event.target as HTMLInputElement).value })"
                  />

                  <select
                    class="object-type-form-input"
                    :value="field.kind"
                    :disabled="field.structuralLocked"
                    @change="
                      emit('updateField', index, {
                        kind: ($event.target as HTMLSelectElement).value as TypeEditorFieldDraft['kind'],
                      })
                    "
                  >
                    <option v-for="option in FIELD_KIND_OPTIONS" :key="option.value" :value="option.value">
                      {{ option.label }}
                    </option>
                  </select>
                </div>

                <div class="object-type-field-row__bottom">
                  <div class="object-type-field-row__modes">
                    <button
                      class="object-type-field-row__mode"
                      :class="{ active: field.displayMode === 'featured' }"
                      type="button"
                      @click="setDisplayMode(index, 'featured')"
                    >
                      В шапке
                    </button>
                    <button
                      class="object-type-field-row__mode"
                      :class="{ active: field.displayMode === 'visible' }"
                      type="button"
                      @click="setDisplayMode(index, 'visible')"
                    >
                      В свойствах
                    </button>
                    <button
                      class="object-type-field-row__mode"
                      :class="{ active: field.displayMode === 'hidden' }"
                      type="button"
                      @click="setDisplayMode(index, 'hidden')"
                    >
                      Скрыто
                    </button>
                  </div>

                  <div class="object-type-field-row__flags">
                    <label class="object-type-field-row__check">
                      <input
                        type="checkbox"
                        :checked="field.read_only === true"
                        :disabled="field.structuralLocked"
                        @change="emit('updateField', index, { read_only: ($event.target as HTMLInputElement).checked })"
                      />
                      <span>Только чтение</span>
                    </label>

                    <label class="object-type-field-row__check">
                      <input
                        type="checkbox"
                        :checked="field.system === true"
                        :disabled="field.structuralLocked"
                        @change="emit('updateField', index, { system: ($event.target as HTMLInputElement).checked })"
                      />
                      <span>Системное</span>
                    </label>
                  </div>

                  <div class="object-type-field-row__actions">
                    <button class="object-type-field-row__ghost" type="button" @click="emit('moveField', index, -1)">
                      Выше
                    </button>
                    <button class="object-type-field-row__ghost" type="button" @click="emit('moveField', index, 1)">
                      Ниже
                    </button>
                    <button
                      class="object-type-field-row__danger"
                      type="button"
                      :disabled="field.structuralLocked"
                      @click="emit('removeField', field.id)"
                    >
                      Удалить
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </article>
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

.object-type-field-row {
  position: relative;
  overflow: hidden;
  border-radius: 18px;
  border: 1px solid transparent;
  background: var(--background);
  transition: border-color 0.16s ease, opacity 0.16s ease, transform 0.16s ease;
}

.object-type-field-row + .object-type-field-row {
  margin-top: 8px;
}

.object-type-field-row__overlay {
  position: absolute;
  inset: 0;
  background: transparent;
  transition: background-color 0.16s ease;
  pointer-events: none;
}

.object-type-field-row:hover,
.object-type-field-row.is-drop-target {
  border-color: var(--border);
}

.object-type-field-row:hover .object-type-field-row__overlay,
.object-type-field-row.is-drop-target .object-type-field-row__overlay {
  background: color-mix(in srgb, var(--secondary) 72%, transparent);
}

.object-type-field-row.is-dragging {
  opacity: 0.54;
  transform: scale(0.992);
}

.object-type-field-row__main {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr);
  gap: 12px;
  align-items: start;
  padding: 14px;
}

.object-type-field-row__drag {
  display: grid;
  grid-template-columns: repeat(2, 4px);
  grid-template-rows: repeat(3, 4px);
  gap: 3px;
  align-content: center;
  justify-content: center;
  width: 28px;
  height: 36px;
  border-radius: 10px;
  background: transparent;
  cursor: grab;
}

.object-type-field-row__drag span {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--muted-foreground);
}

.object-type-field-row__content {
  display: grid;
  gap: 10px;
}

.object-type-field-row__topline {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.object-type-field-row__kind,
.object-type-field-row__id {
  display: inline-flex;
  align-items: center;
  min-height: 24px;
  padding: 0 8px;
  border-radius: 999px;
  font-size: 11px;
}

.object-type-field-row__kind {
  background: var(--secondary);
  color: var(--secondary-foreground);
  font-weight: 600;
}

.object-type-field-row__id {
  border: 1px solid var(--border);
  background: var(--background);
  color: var(--muted-foreground);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

.object-type-field-row__identity {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 200px;
  gap: 10px;
}

.object-type-form-input {
  width: 100%;
  min-height: 40px;
  padding: 0 12px;
  border: 1px solid var(--input);
  border-radius: 12px;
  background: var(--background);
  color: var(--foreground);
  font-size: 13px;
  transition: border-color 0.16s ease, box-shadow 0.16s ease, background-color 0.16s ease;
}

.object-type-form-input:hover {
  background: color-mix(in srgb, var(--background) 82%, var(--secondary));
}

.object-type-form-input:focus {
  outline: none;
  border-color: var(--ring);
  box-shadow: 0 0 0 1px var(--ring);
}

.object-type-form-input--title {
  font-size: 15px;
  font-weight: 600;
}

.object-type-field-row__bottom {
  display: flex;
  align-items: center;
  gap: 14px;
  flex-wrap: wrap;
}

.object-type-field-row__modes {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.object-type-field-row__mode {
  min-height: 28px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: transparent;
  color: var(--muted-foreground);
  font-size: 12px;
  transition: background-color 0.16s ease, border-color 0.16s ease, color 0.16s ease;
}

.object-type-field-row__mode.active {
  border-color: var(--foreground);
  background: var(--foreground);
  color: var(--background);
}

.object-type-field-row__flags {
  display: inline-flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.object-type-field-row__check {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: var(--muted-foreground);
  font-size: 12px;
}

.object-type-field-row__actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
  opacity: 0;
  transition: opacity 0.16s ease;
}

.object-type-field-row:hover .object-type-field-row__actions,
.object-type-field-row.is-drop-target .object-type-field-row__actions {
  opacity: 1;
}

.object-type-field-row__ghost,
.object-type-field-row__danger {
  min-height: 30px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--background);
  color: var(--foreground);
  font-size: 12px;
}

.object-type-field-row__danger {
  border-color: var(--destructive);
  color: var(--destructive);
}

@media (max-width: 1100px) {
  .object-type-section__head,
  .object-type-field-bucket__head {
    flex-direction: column;
  }

  .object-type-field-row__identity {
    grid-template-columns: 1fr;
  }

  .object-type-field-row__actions {
    width: 100%;
    margin-left: 0;
    opacity: 1;
  }
}
</style>
