<template>
  <div class="typed-note-shell">
    <div
      v-if="activeNoteType && noteTypeDefinition"
      class="typed-note-hero"
      :class="`typed-note-hero-${headerTemplate?.kind ?? 'default'}`"
      data-testid="typed-note-header"
    >
      <!-- Centered profile layout -->
      <template v-if="headerTemplate?.kind === 'centered_profile'">
        <div class="typed-note-avatar-wrap">
          <img
            v-if="imageSrc"
            class="typed-note-avatar"
            data-testid="typed-note-avatar"
            :src="imageSrc"
            :alt="title || activeNoteType.name"
          />
          <div v-else class="typed-note-avatar typed-note-avatar-placeholder">
            {{ activeNoteType.icon ?? "✦" }}
          </div>
        </div>
        <div class="typed-note-hero-text">
          <h2 class="typed-note-hero-title" data-testid="typed-note-primary">
            {{ primaryText || title || activeNoteType.name }}
          </h2>
          <p v-if="secondaryText" class="typed-note-hero-subtitle">{{ secondaryText }}</p>
        </div>
      </template>

      <!-- Default layout -->
      <div v-else-if="headerTemplate?.kind === 'default'" class="typed-note-hero-default">
        <h2 class="typed-note-hero-title">{{ title || primaryText || activeNoteType.name }}</h2>
        <p v-if="secondaryText" class="typed-note-hero-subtitle">{{ secondaryText }}</p>
      </div>

      <!-- Fields grid -->
      <div class="typed-note-fields-grid">
        <template v-for="field in noteTypeDefinition.fields" :key="field.id">
          <!-- Boolean -->
          <label v-if="field.kind === 'boolean'" class="typed-note-field">
            <span>{{ field.label }}</span>
            <label class="typed-note-checkbox">
              <input
                type="checkbox"
                :checked="headerProps[field.id] === true"
                @change="
                  emit('headerPropChange', field.id, ($event.target as HTMLInputElement).checked)
                "
              />
              <span>{{ headerProps[field.id] === true ? "Да" : "Нет" }}</span>
            </label>
          </label>

          <!-- Long text -->
          <label
            v-else-if="field.kind === 'long_text'"
            class="typed-note-field typed-note-field-wide"
          >
            <span>{{ field.label }}</span>
            <textarea
              class="typed-note-input typed-note-textarea"
              :value="String(headerProps[field.id] ?? '')"
              :placeholder="field.placeholder ?? ''"
              @input="
                emit('headerPropChange', field.id, ($event.target as HTMLTextAreaElement).value)
              "
            />
          </label>

          <!-- Select -->
          <label v-else-if="field.kind === 'select'" class="typed-note-field">
            <span>{{ field.label }}</span>
            <select
              class="typed-note-input"
              :value="String(headerProps[field.id] ?? '')"
              @change="
                emit('headerPropChange', field.id, ($event.target as HTMLSelectElement).value)
              "
            >
              <option value="">Не выбрано</option>
              <option v-for="option in field.options ?? []" :key="option" :value="option">
                {{ option }}
              </option>
            </select>
          </label>

          <!-- Generic input -->
          <label v-else class="typed-note-field">
            <span>{{ field.label }}</span>
            <input
              class="typed-note-input"
              :data-testid="`typed-note-field-${field.id}`"
              :type="
                field.kind === 'number'
                  ? 'number'
                  : field.kind === 'date'
                    ? 'date'
                    : field.kind === 'image'
                      ? 'url'
                      : 'text'
              "
              :value="String(headerProps[field.id] ?? '')"
              :placeholder="field.placeholder ?? ''"
              @input="emit('headerPropChange', field.id, ($event.target as HTMLInputElement).value)"
            />
          </label>
        </template>
      </div>

      <div v-if="validationError" class="typed-note-error">{{ validationError }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { parseHeaderTemplate, parseNoteTypeDefinition } from "@/lib/typedNotes";

const props = defineProps<{
  activeNoteType: NoteType | null;
  title: string;
  headerProps: Record<string, unknown>;
  validationError: string | null;
}>();

const emit = defineEmits<{
  headerPropChange: [fieldId: string, value: unknown];
}>();

const noteTypeDefinition = computed(() =>
  props.activeNoteType ? parseNoteTypeDefinition(props.activeNoteType.schema_json) : null,
);
const headerTemplate = computed(() =>
  props.activeNoteType ? parseHeaderTemplate(props.activeNoteType.header_template_json) : null,
);

const primaryText = computed(() => {
  const ids = headerTemplate.value?.primaryFieldIds ?? [];
  return ids
    .map((id) => String(props.headerProps[id] ?? "").trim())
    .filter(Boolean)
    .join(" ");
});

const secondaryText = computed(() => {
  const ids = headerTemplate.value?.secondaryFieldIds ?? [];
  return ids
    .map((id) => String(props.headerProps[id] ?? "").trim())
    .filter(Boolean)
    .join(" · ");
});

const imageSrc = computed(() => {
  const id = headerTemplate.value?.imageFieldId ?? null;
  return id ? String(props.headerProps[id] ?? "").trim() : "";
});
</script>
