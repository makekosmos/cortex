<script setup lang="ts">
import { computed } from "vue";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { formatObjectFieldValue } from "@/lib/objectFieldFormatting";
import type { ResolvedNoteTypeField } from "@/lib/typedNotes";
import ObjectPropertyPicker from "./ObjectPropertyPicker.vue";

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
const isSelectLike = computed(() =>
  props.field.kind === "select" ||
  props.field.kind === "multi_select" ||
  props.field.kind === "relation",
);
const usesCustomPicker = computed(() => isSelectLike.value);

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

const selectedValues = computed(() =>
  Array.isArray(props.modelValue)
    ? props.modelValue.filter((value): value is string => typeof value === "string")
    : typeof props.modelValue === "string"
      ? [props.modelValue]
      : [],
);

const relationIds = computed(() =>
  props.field.kind === "relation" ? selectedValues.value : [],
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

const pickerOptions = computed(() => {
  if (props.field.kind === "relation") {
    const options = filteredRelationCandidates.value.map((entry) => ({
      value: entry.id,
      label: getEntryDisplayTitle(entry.title, entry.header_props_json),
    }));
    const knownIds = new Set(options.map((option) => option.value));

    for (const relationId of relationIds.value) {
      if (knownIds.has(relationId)) {
        continue;
      }

      const entry = props.entriesById.get(relationId);
      options.push({
        value: relationId,
        label: entry ? getEntryDisplayTitle(entry.title, entry.header_props_json) : relationId,
      });
    }

    return options;
  }

  return (props.field.options ?? []).map((option) => ({
    value: option,
    label: formatOptionLabel(option),
  }));
});

const displayValue = computed(() => {
  if (props.field.kind === "relation") {
    return relationIds.value
      .map((id) => {
        const entry = props.entriesById.get(id);
        return entry ? getEntryDisplayTitle(entry.title, entry.header_props_json) : id;
      })
      .join(", ");
  }

  return formatObjectFieldValue(props.field, props.modelValue);
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
      return "Выбрать вариант";
    case "multi_select":
      return "Выбрать варианты";
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

function formatOptionLabel(option: string) {
  return formatObjectFieldValue(props.field, option) || option;
}
</script>

<template>
  <div
    class="object-property-field"
    :class="[
      `object-property-field--${layout}`,
      `object-property-field--${variant}`,
      isReadonly && 'object-property-field--readonly',
      isSelectLike && 'object-property-field--selectlike',
      field.kind === 'long_text' && 'object-property-field--wide',
    ]"
  >
    <div class="object-property-field__label">
      <span class="object-property-field__label-text">{{ field.label }}</span>
      <span v-if="isReadonly" class="object-property-field__lock" aria-hidden="true">🔒</span>
    </div>

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
        <span>{{ formatObjectFieldValue(field, modelValue) }}</span>
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

      <div v-else-if="field.kind === 'relation'" class="object-property-field__relation">
        <div v-if="relationIds.length > 0 && isReadonly" class="object-property-field__chips">
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
        </div>

        <div
          v-else-if="relationIds.length === 0 && isReadonly"
          class="object-property-field__empty"
        >
          Нет связей
        </div>

        <ObjectPropertyPicker
          v-else
          :data-testid="`typed-note-field-${field.id}`"
          :model-value="selectedValues"
          :options="pickerOptions"
          :placeholder="inputPlaceholder"
          :variant="variant"
          multiple
          empty-options-label="Нет доступных объектов"
          @update:model-value="emit('update:modelValue', $event)"
        />
      </div>

      <ObjectPropertyPicker
        v-else-if="usesCustomPicker"
        :data-testid="`typed-note-field-${field.id}`"
        :model-value="field.kind === 'multi_select' ? selectedValues : String(modelValue ?? '')"
        :options="pickerOptions"
        :placeholder="inputPlaceholder"
        :variant="variant"
        :multiple="field.kind === 'multi_select'"
        :disabled="isReadonly"
        @update:model-value="emit('update:modelValue', $event)"
      />

      <input
        v-else
        class="object-property-field__input"
        :class="isSelectLike && 'object-property-field__input--textual'"
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
  gap: 8px;
  min-height: 28px;
  padding: 2px 8px;
  border-radius: 8px;
  transition: background-color 0.16s ease;
}

.object-property-field--featured-inline:hover {
  background: color-mix(in srgb, var(--secondary) 72%, transparent);
}

.object-property-field--featured-column,
.object-property-field--secondary {
  display: grid;
  grid-template-columns: minmax(0, 30%) minmax(0, 70%);
  gap: 14px;
  align-items: stretch;
  width: 100%;
}

.object-property-field--secondary {
  position: relative;
  min-height: 40px;
  padding: 8px 10px;
  border-radius: 10px;
  transition: background-color 0.16s ease;
}

.object-property-field--secondary:hover {
  background: color-mix(in srgb, var(--background) 82%, var(--secondary));
}

.object-property-field--secondary.object-property-field--wide {
  grid-template-columns: 1fr;
}

.object-property-field__label {
  color: var(--muted-foreground);
  font-size: 13px;
  line-height: 1.4;
  display: flex;
  align-items: center;
  gap: 6px;
  text-align: left;
}

.object-property-field__label-text {
  min-width: 0;
}

.object-property-field__lock {
  flex-shrink: 0;
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1;
  opacity: 0.84;
}

.object-property-field--featured-inline .object-property-field__label {
  white-space: nowrap;
}

.object-property-field__value {
  display: flex;
  align-items: center;
  min-width: 0;
  width: 100%;
  min-height: 24px;
  color: var(--foreground);
  font-size: 13px;
  line-height: 1.4;
  text-align: left;
}

.object-property-field--featured-column .object-property-field__label,
.object-property-field--featured-column .object-property-field__value,
.object-property-field--secondary .object-property-field__label,
.object-property-field--secondary .object-property-field__value {
  align-items: flex-start;
}

.object-property-field__read {
  width: 100%;
  text-align: left;
  white-space: normal;
  overflow-wrap: anywhere;
}

.object-property-field__read--placeholder {
  color: var(--muted-foreground);
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

.object-property-field__input--textual {
  min-height: 28px;
  padding: 0;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
  appearance: none;
  -webkit-appearance: none;
}

.object-property-field__input--textual:hover {
  background: transparent;
}

.object-property-field__input:focus {
  outline: none;
  border-color: var(--ring);
  box-shadow: 0 0 0 1px var(--ring);
}

.object-property-field__input--textual:focus {
  border-color: transparent;
  box-shadow: none;
}

.object-property-field__textarea {
  min-height: 84px;
  resize: vertical;
}

.object-property-field__checkbox {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: inherit;
}

.object-property-field__relation {
  width: 100%;
  min-width: 0;
  display: flex;
  align-items: flex-start;
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
  min-height: 24px;
  padding: 0 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--secondary);
  color: var(--secondary-foreground);
  font-size: 13px;
}

.object-property-field__chip {
  cursor: pointer;
}
</style>
