<script setup lang="ts">
import { computed } from "vue";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import type { ResolvedNoteTypeField } from "@/lib/typedNotes";

const props = withDefaults(defineProps<{
  field: ResolvedNoteTypeField;
  modelValue: unknown;
  layout: "inline" | "column";
  relationCandidates: Entry[];
  entriesById: Map<string, Entry>;
  readonly?: boolean;
  variant?: "featured-inline" | "featured-column" | "secondary";
}>(), {
  variant: "secondary",
});

const emit = defineEmits<{
  "update:modelValue": [value: unknown];
  relationNavigate: [entryId: string];
}>();

const isReadonly = computed(() => props.readonly === true || props.field.read_only);
const isCompactMatrix = computed(() => props.variant === "secondary" || props.variant === "featured-column");
const inputType = computed(() => {
  switch (props.field.kind) {
    case "number":
      return "number";
    case "date":
      return "date";
    case "image":
      return "url";
    default:
      return "text";
  }
});

const relationIds = computed(() =>
  Array.isArray(props.modelValue)
    ? props.modelValue.filter((value): value is string => typeof value === "string")
    : [],
);

const selectedValues = computed(() =>
  Array.isArray(props.modelValue)
    ? props.modelValue.filter((value): value is string => typeof value === "string")
    : typeof props.modelValue === "string"
      ? [props.modelValue]
      : [],
);

const filteredRelationCandidates = computed(() => {
  const allowedTypes = props.field.allowed_object_types?.filter(Boolean) ?? [];
  if (allowedTypes.length === 0) {
    return props.relationCandidates;
  }

  return props.relationCandidates.filter((entry) =>
    entry.type_id ? allowedTypes.includes(entry.type_id) : false,
  );
});

const displayValue = computed(() => {
  if (props.field.kind === "boolean") {
    return props.modelValue === true ? "Да" : "Нет";
  }

  if (props.field.kind === "multi_select") {
    return Array.isArray(props.modelValue) ? props.modelValue.join(", ") : "";
  }

  if (props.field.kind === "relation") {
    return relationIds.value
      .map((id) => {
        const entry = props.entriesById.get(id);
        return entry ? getEntryDisplayTitle(entry.title, entry.header_props_json) : id;
      })
      .join(", ");
  }

  return String(props.modelValue ?? "");
});

const inputPlaceholder = computed(() => {
  if (props.field.placeholder) {
    return props.field.placeholder;
  }

  switch (props.field.kind) {
    case "number":
      return "Введите число";
    case "date":
      return "Выберите дату";
    case "select":
    case "multi_select":
      return "Выбрать вариант";
    case "relation":
      return "Выберите объекты";
    case "image":
      return "Вставьте ссылку";
    default:
      return "Введите значение";
  }
});

function updateTextValue(event: Event) {
  emit("update:modelValue", (event.target as HTMLInputElement).value);
}

function updateTextareaValue(event: Event) {
  emit("update:modelValue", (event.target as HTMLTextAreaElement).value);
}

function updateBooleanValue(event: Event) {
  emit("update:modelValue", (event.target as HTMLInputElement).checked);
}

function updateSelectValue(event: Event) {
  const select = event.target as HTMLSelectElement;
  if (props.field.kind === "multi_select" || props.field.kind === "relation") {
    emit(
      "update:modelValue",
      Array.from(select.selectedOptions, (option) => option.value),
    );
    return;
  }

  emit("update:modelValue", select.value);
}
</script>

<template>
  <div
    class="object-property-field"
    :class="[
      `object-property-field--${layout}`,
      `object-property-field--${variant}`,
      isCompactMatrix && 'object-property-field--compact',
      isReadonly && 'object-property-field--readonly',
      field.kind === 'long_text' && 'object-property-field--wide',
    ]"
  >
    <div class="object-property-field__label">{{ field.label }}</div>

    <div class="object-property-field__value">
      <template v-if="isReadonly && field.kind !== 'relation'">
        <div class="object-property-field__read">{{ displayValue || "—" }}</div>
      </template>

      <label v-else-if="field.kind === 'boolean'" class="object-property-field__checkbox">
        <input
          type="checkbox"
          :checked="modelValue === true"
          :disabled="isReadonly"
          @change="updateBooleanValue"
        />
        <span>{{ modelValue === true ? "Да" : "Нет" }}</span>
      </label>

      <textarea
        v-else-if="field.kind === 'long_text'"
        class="object-property-field__input object-property-field__textarea"
        :data-testid="`typed-note-field-${field.id}`"
        :value="String(modelValue ?? '')"
        :placeholder="inputPlaceholder"
        :disabled="isReadonly"
        @input="updateTextareaValue"
      />

      <div v-else-if="field.kind === 'relation'" class="object-property-field__chips">
        <button
          v-for="relationId in relationIds"
          :key="relationId"
          class="object-property-field__chip"
          type="button"
          @click="emit('relationNavigate', relationId)"
        >
          {{
            entriesById.has(relationId)
              ? getEntryDisplayTitle(
                  entriesById.get(relationId)?.title,
                  entriesById.get(relationId)?.header_props_json,
                )
              : relationId
          }}
        </button>
        <div v-if="relationIds.length === 0 && isReadonly" class="object-property-field__empty">
          Нет связей
        </div>

        <select
          v-if="!isReadonly"
          class="object-property-field__input"
          :data-testid="`typed-note-field-${field.id}`"
          multiple
          :size="isCompactMatrix ? 1 : variant !== 'featured-inline' ? Math.min(Math.max(filteredRelationCandidates.length, 2), 6) : undefined"
          @change="updateSelectValue"
        >
          <option
            v-for="entry in filteredRelationCandidates"
            :key="entry.id"
            :value="entry.id"
            :selected="selectedValues.includes(entry.id)"
          >
            {{ getEntryDisplayTitle(entry.title, entry.header_props_json) }}
          </option>
        </select>
      </div>

      <select
        v-else-if="field.kind === 'select' || field.kind === 'multi_select'"
        class="object-property-field__input"
        :data-testid="`typed-note-field-${field.id}`"
        :multiple="field.kind === 'multi_select'"
        :disabled="isReadonly"
        @change="updateSelectValue"
      >
        <option
          v-if="field.kind === 'select'"
          value=""
          :selected="String(modelValue ?? '') === ''"
        >
          {{ inputPlaceholder }}
        </option>

        <option
          v-for="option in field.options ?? []"
          :key="option"
          :value="option"
          :selected="selectedValues.includes(option)"
        >
          {{ option }}
        </option>
      </select>

      <input
        v-else
        class="object-property-field__input"
        :data-testid="`typed-note-field-${field.id}`"
        :type="inputType"
        :value="String(modelValue ?? '')"
        :placeholder="inputPlaceholder"
        :disabled="isReadonly"
        @input="updateTextValue"
      />
    </div>
  </div>
</template>

<style scoped>
.object-property-field {
  min-width: 0;
}

.object-property-field--featured-inline {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 28px;
  position: relative;
  padding: 2px 12px 2px 8px;
  border-radius: 8px;
  transition: background-color 0.16s ease;
}

.object-property-field--featured-inline:hover {
  background: color-mix(in srgb, var(--secondary) 72%, transparent);
}

.object-property-field--featured-inline::after {
  content: "";
  position: absolute;
  right: 4px;
  top: 50%;
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: var(--muted-foreground);
  transform: translateY(-50%);
}

.object-property-field--featured-inline:last-child::after {
  display: none;
}

.object-property-field--featured-column,
.object-property-field--secondary {
  display: grid;
  grid-template-columns: minmax(136px, 168px) minmax(0, 1fr);
  gap: 12px;
  align-items: start;
}

.object-property-field--secondary {
  position: relative;
  padding: 4px 0;
  border-radius: 0;
  transition: background-color 0.16s ease;
}

.object-property-field--compact:hover {
  background: color-mix(in srgb, var(--background) 82%, var(--secondary));
}

.object-property-field--secondary.object-property-field--wide {
  grid-template-columns: 1fr;
}

.object-property-field__label {
  color: var(--muted-foreground);
  font-size: 13px;
  line-height: 1.4;
}

.object-property-field--featured-inline .object-property-field__label {
  white-space: nowrap;
}

.object-property-field--compact .object-property-field__label {
  padding-top: 7px;
}

.object-property-field__value {
  min-width: 0;
}

.object-property-field__read {
  color: var(--foreground);
  font-size: 13px;
  line-height: 1.5;
}

.object-property-field--featured-inline .object-property-field__read {
  font-size: 13px;
}

.object-property-field__input {
  width: 100%;
  min-height: 34px;
  padding: 6px 10px;
  border: 1px solid var(--input);
  border-radius: 10px;
  background: var(--background);
  color: var(--foreground);
  font: inherit;
  transition: border-color 0.16s ease, box-shadow 0.16s ease, background-color 0.16s ease;
}

.object-property-field__input:hover {
  background: color-mix(in srgb, var(--background) 82%, var(--secondary));
}

.object-property-field--compact .object-property-field__input {
  min-height: 32px;
  padding: 5px 8px;
  border: none;
  border-radius: 6px;
  background: transparent;
  box-shadow: none;
}

.object-property-field--compact .object-property-field__input:hover {
  background: color-mix(in srgb, var(--background) 78%, var(--secondary));
}

.object-property-field--featured-inline .object-property-field__input {
  min-width: 110px;
}

.object-property-field__input:focus {
  outline: none;
  border-color: var(--ring);
  box-shadow: 0 0 0 1px var(--ring);
}

.object-property-field--compact .object-property-field__input:focus {
  background: color-mix(in srgb, var(--background) 74%, var(--secondary));
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--ring) 72%, transparent);
}

.object-property-field__textarea {
  min-height: 84px;
  resize: vertical;
}

.object-property-field--compact .object-property-field__textarea {
  min-height: 36px;
}

.object-property-field__checkbox {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: var(--foreground);
}

.object-property-field__chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.object-property-field__chip,
.object-property-field__empty {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  padding: 0 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--secondary);
  color: var(--secondary-foreground);
  font-size: 12px;
}

.object-property-field__chip {
  cursor: pointer;
}
</style>
