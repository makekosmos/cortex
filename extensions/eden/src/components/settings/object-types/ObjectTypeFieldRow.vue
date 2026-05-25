<script setup lang="ts">
// ObjectTypeFieldRow — одна строка поля в drag-drop списке внутри
// ObjectTypeFieldsSection. Раньше эти 130+ строк template'a жили inline
// в v-for, что делало parent template нечитаемым. Теперь parent передаёт
// данные/состояние и обрабатывает emits.

import type { TypeEditorFieldDraft } from "./shared";
import { FIELD_KIND_OPTIONS } from "./shared";

const FIELD_KIND_LABELS = new Map(
  FIELD_KIND_OPTIONS.map((option) => [option.value, option.label]),
);

defineProps<{
  field: TypeEditorFieldDraft;
  index: number;
  isDragging: boolean;
  isDropTarget: boolean;
}>();

const emit = defineEmits<{
  updateField: [index: number, patch: Partial<TypeEditorFieldDraft>];
  moveField: [index: number, direction: -1 | 1];
  removeField: [fieldId: string];
  setDisplayMode: [index: number, displayMode: TypeEditorFieldDraft["displayMode"]];
  dragStart: [index: number];
  dragEnter: [index: number];
  dragEnd: [];
}>();
</script>

<template>
  <article
    class="object-type-field-row"
    :class="{
      'is-dragging': isDragging,
      'is-drop-target': isDropTarget,
    }"
    :draggable="!field.structuralLocked"
    @dragstart="emit('dragStart', index)"
    @dragenter.prevent="emit('dragEnter', index)"
    @dragover.prevent
    @dragend="emit('dragEnd')"
    @drop.prevent="emit('dragEnd')"
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
          <div class="object-type-field-row__kind">
            {{ FIELD_KIND_LABELS.get(field.kind) }}
          </div>
          <div class="object-type-field-row__id">{{ field.id }}</div>
        </div>

        <div class="object-type-field-row__identity">
          <input
            class="object-type-form-input object-type-form-input--title"
            :value="field.label"
            :disabled="field.structuralLocked"
            placeholder="Название поля"
            @input="
              emit('updateField', index, {
                label: ($event.target as HTMLInputElement).value,
              })
            "
          />

          <select
            class="object-type-form-input"
            :value="field.kind"
            :disabled="field.structuralLocked"
            @change="
              emit('updateField', index, {
                kind: ($event.target as HTMLSelectElement)
                  .value as TypeEditorFieldDraft['kind'],
              })
            "
          >
            <option
              v-for="option in FIELD_KIND_OPTIONS"
              :key="option.value"
              :value="option.value"
            >
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
              @click="emit('setDisplayMode', index, 'featured')"
            >
              В шапке
            </button>
            <button
              class="object-type-field-row__mode"
              :class="{ active: field.displayMode === 'visible' }"
              type="button"
              @click="emit('setDisplayMode', index, 'visible')"
            >
              В свойствах
            </button>
            <button
              class="object-type-field-row__mode"
              :class="{ active: field.displayMode === 'hidden' }"
              type="button"
              @click="emit('setDisplayMode', index, 'hidden')"
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
                @change="
                  emit('updateField', index, {
                    read_only: ($event.target as HTMLInputElement).checked,
                  })
                "
              />
              <span>Только чтение</span>
            </label>

            <label class="object-type-field-row__check">
              <input
                type="checkbox"
                :checked="field.system === true"
                :disabled="field.structuralLocked"
                @change="
                  emit('updateField', index, {
                    system: ($event.target as HTMLInputElement).checked,
                  })
                "
              />
              <span>Системное</span>
            </label>
          </div>

          <div class="object-type-field-row__actions">
            <button
              class="object-type-field-row__ghost"
              type="button"
              @click="emit('moveField', index, -1)"
            >
              Выше
            </button>
            <button
              class="object-type-field-row__ghost"
              type="button"
              @click="emit('moveField', index, 1)"
            >
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
</template>

<style scoped>
/* Field-row CSS — мигрировано из parent ObjectTypeFieldsSection. */
.object-type-field-row {
  position: relative;
  overflow: hidden;
  border-radius: 18px;
  border: 1px solid transparent;
  background: var(--background);
  transition:
    border-color 0.16s ease,
    opacity 0.16s ease,
    transform 0.16s ease;
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
  transition:
    border-color 0.16s ease,
    box-shadow 0.16s ease,
    background-color 0.16s ease;
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
  transition:
    background-color 0.16s ease,
    border-color 0.16s ease,
    color 0.16s ease;
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
