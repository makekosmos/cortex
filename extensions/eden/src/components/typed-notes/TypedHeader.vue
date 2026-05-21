<script setup lang="ts">
import { computed } from "vue";
import ObjectPropertyField from "./ObjectPropertyField.vue";
import { UNTITLED_ENTRY_PLACEHOLDER } from "@/lib/entryTitles";
import { getNoteTypePresentation, getResolvedNoteTypeField } from "@/lib/typedNotes";
import { SYSTEM_TYPE_GAME_ID, SYSTEM_TYPE_NOTE_ID } from "@/lib/systemTypes";

const props = withDefaults(
  defineProps<{
    activeNoteType: NoteType | null;
    title: string;
    headerProps: Record<string, unknown>;
    validationError: string | null;
    allEntries: Entry[];
    currentEntryId: string;
    readonly?: boolean;
    showTitle?: boolean;
    showTypeRow?: boolean;
  }>(),
  {
    showTitle: false,
    showTypeRow: true,
  },
);

const emit = defineEmits<{
  headerPropChange: [fieldId: string, value: unknown];
  relationNavigate: [entryId: string];
}>();

const presentation = computed(() => getNoteTypePresentation(props.activeNoteType));
const entriesById = computed(
  () => new Map(props.allEntries.map((entry) => [entry.id, entry] satisfies [string, Entry])),
);
const relationCandidates = computed(() =>
  props.allEntries.filter((entry) => entry.id !== props.currentEntryId),
);

const descriptionValue = computed(() => {
  const fieldId = presentation.value.descriptionField?.id;
  return fieldId ? String(props.headerProps[fieldId] ?? "") : "";
});

function hasMeaningfulValue(fieldId: string) {
  const value = props.headerProps[fieldId];

  if (Array.isArray(value)) {
    return value.length > 0;
  }

  if (typeof value === "boolean") {
    return value;
  }

  if (typeof value === "number") {
    return Number.isFinite(value);
  }

  return String(value ?? "").trim().length > 0;
}

const coverImageSrc = computed(() => {
  const imageFieldId = presentation.value.imageFieldId;
  if (!imageFieldId) {
    return "";
  }

  return String(props.headerProps[imageFieldId] ?? "").trim();
});
const backgroundImageSrc = computed(() => {
  const field = getResolvedNoteTypeField(props.activeNoteType, "background_image");
  if (!field?.visible) {
    return "";
  }

  return String(props.headerProps.background_image ?? "").trim();
});
const titleText = computed(
  () => props.title.trim() || props.activeNoteType?.name || UNTITLED_ENTRY_PLACEHOLDER,
);
const isGameNoteType = computed(() => props.activeNoteType?.id === SYSTEM_TYPE_GAME_ID);
const isPlainNoteType = computed(() => props.activeNoteType?.id === SYSTEM_TYPE_NOTE_ID);
const shouldShowDescription = computed(() => {
  if (!presentation.value.descriptionField) {
    return false;
  }

  if (props.readonly) {
    return Boolean(descriptionValue.value.trim());
  }

  return !isPlainNoteType.value || Boolean(descriptionValue.value.trim());
});
const showVisual = computed(() => Boolean(coverImageSrc.value));
const allowEmptyHeaderFields = computed(() => !isPlainNoteType.value);
const renderedFeaturedFields = computed(() =>
  isGameNoteType.value
    ? []
    : presentation.value.featuredFields.filter(
        (field) => allowEmptyHeaderFields.value || hasMeaningfulValue(field.id),
      ),
);
const renderedSecondaryFields = computed(() =>
  (isGameNoteType.value
    ? [...presentation.value.featuredFields, ...presentation.value.secondaryFields]
    : presentation.value.secondaryFields
  ).filter((field) => allowEmptyHeaderFields.value || hasMeaningfulValue(field.id)),
);
const hasFeaturedFields = computed(() => renderedFeaturedFields.value.length > 0);
const hasSecondaryFields = computed(() => renderedSecondaryFields.value.length > 0);
const hasHeroContent = computed(
  () =>
    props.showTitle || shouldShowDescription.value || hasFeaturedFields.value || showVisual.value,
);
const shouldRenderHeader = computed(
  () =>
    Boolean(props.activeNoteType) &&
    (props.showTypeRow ||
      hasHeroContent.value ||
      hasSecondaryFields.value ||
      Boolean(props.validationError) ||
      Boolean(backgroundImageSrc.value)),
);

function updateDescription(event: Event) {
  const fieldId = presentation.value.descriptionField?.id;
  if (!fieldId) {
    return;
  }

  emit("headerPropChange", fieldId, (event.target as HTMLTextAreaElement).value);
}
</script>

<template>
  <section
    v-if="shouldRenderHeader && activeNoteType"
    class="typed-object-header"
    :class="`typed-object-header--${presentation.headerLayout}`"
    data-testid="typed-note-header"
  >
    <div
      v-if="backgroundImageSrc"
      class="typed-object-header__background"
      :style="{ backgroundImage: `url(${backgroundImageSrc})` }"
      aria-hidden="true"
    />

    <div class="typed-object-header__inner">
      <div v-if="showTypeRow" class="typed-object-header__type-row">
        <span class="typed-object-header__type-badge">{{ activeNoteType.name }}</span>
      </div>

      <div
        v-if="hasHeroContent"
        class="typed-object-header__hero"
        :class="[
          `typed-object-header__hero--${presentation.headerLayout}`,
          !showVisual && 'typed-object-header__hero--no-visual',
        ]"
      >
        <div v-if="showVisual" class="typed-object-header__visual">
          <img
            v-if="coverImageSrc"
            class="typed-object-header__cover"
            :src="coverImageSrc"
            :alt="titleText"
          />
        </div>

        <div class="typed-object-header__content">
          <h2 v-if="showTitle" class="typed-object-header__title">{{ titleText }}</h2>

          <div v-if="shouldShowDescription" class="typed-object-header__description-wrap">
            <p
              v-if="readonly"
              class="typed-object-header__description typed-object-header__description--readonly"
            >
              {{ descriptionValue }}
            </p>

            <textarea
              v-else
              class="typed-object-header__description"
              :value="descriptionValue"
              :placeholder="presentation.descriptionField.placeholder ?? 'Краткое описание объекта'"
              :disabled="presentation.descriptionField.read_only"
              @input="updateDescription"
            />
          </div>

          <div
            v-if="hasFeaturedFields"
            class="typed-object-header__featured"
            :class="`typed-object-header__featured--${presentation.headerLayout}`"
          >
            <ObjectPropertyField
              v-for="field in renderedFeaturedFields"
              :key="field.id"
              :field="field"
              :model-value="headerProps[field.id]"
              :layout="presentation.headerLayout"
              :variant="
                presentation.headerLayout === 'column' ? 'featured-column' : 'featured-inline'
              "
              :relation-candidates="relationCandidates"
              :entries-by-id="entriesById"
              :readonly="readonly"
              @update:model-value="emit('headerPropChange', field.id, $event)"
              @relation-navigate="emit('relationNavigate', $event)"
            />
          </div>
        </div>
      </div>

      <div v-if="hasSecondaryFields" class="typed-object-header__secondary">
        <div class="typed-object-header__secondary-list">
          <ObjectPropertyField
            v-for="field in renderedSecondaryFields"
            :key="field.id"
            :field="field"
            :model-value="headerProps[field.id]"
            layout="column"
            variant="secondary"
            :relation-candidates="relationCandidates"
            :entries-by-id="entriesById"
            :readonly="readonly"
            @update:model-value="emit('headerPropChange', field.id, $event)"
            @relation-navigate="emit('relationNavigate', $event)"
          />
        </div>
      </div>

      <div v-if="validationError" class="typed-object-header__error">
        {{ validationError }}
      </div>
    </div>
  </section>
</template>

<style scoped>
.typed-object-header {
  position: relative;
  margin: 6px 0 0;
  width: 100%;
  overflow: hidden;
}

.typed-object-header__background {
  position: absolute;
  inset: 0;
  background-position: center;
  background-size: cover;
  opacity: 0.08;
  filter: saturate(0.9) blur(2px);
}

.typed-object-header__inner {
  position: relative;
  display: grid;
  gap: 12px;
  width: 100%;
  padding: 2px 0 4px;
}

.typed-object-header__type-row {
  display: flex;
  align-items: center;
}

.typed-object-header__type-badge {
  display: inline-flex;
  align-items: center;
  min-height: 24px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--secondary);
  color: var(--secondary-foreground);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.typed-object-header__hero {
  display: grid;
  gap: 14px;
  width: 100%;
}

.typed-object-header__hero--column {
  grid-template-columns: 104px minmax(0, 1fr);
  align-items: start;
}

.typed-object-header__hero--inline {
  grid-template-columns: minmax(88px, 96px) minmax(0, 1fr);
  align-items: start;
}

.typed-object-header__hero--no-visual {
  grid-template-columns: minmax(0, 1fr);
}

.typed-object-header__visual {
  display: flex;
  align-items: flex-start;
  justify-content: flex-start;
}

.typed-object-header__cover {
  width: 100%;
  aspect-ratio: 1 / 1;
  border-radius: 20px;
}

.typed-object-header__cover {
  display: block;
  object-fit: cover;
}

.typed-object-header__content {
  display: grid;
  gap: 10px;
  width: 100%;
  min-width: 0;
}

.typed-object-header__title {
  margin: 0;
  color: var(--foreground);
  font-size: 36px;
  line-height: 1.02;
  font-weight: 600;
  letter-spacing: -0.05em;
}

.typed-object-header__description-wrap {
  max-width: 680px;
}

.typed-object-header__description {
  width: 100%;
  min-height: 52px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--muted-foreground);
  font: inherit;
  font-size: 15px;
  line-height: 1.68;
  resize: vertical;
}

.typed-object-header__description:focus {
  outline: none;
}

.typed-object-header__description--readonly {
  margin: 0;
}

.typed-object-header__featured--inline {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
  margin-left: -8px;
}

.typed-object-header__featured--column {
  display: grid;
  gap: 10px;
  max-width: 680px;
  padding-top: 10px;
}

.typed-object-header__secondary {
  display: grid;
  gap: 4px;
  width: 100%;
  padding-top: 4px;
}

.typed-object-header__secondary-list {
  display: grid;
  gap: 0;
  width: 100%;
}

.typed-object-header__error {
  padding: 12px 14px;
  border: 1px solid var(--destructive);
  border-radius: 12px;
  color: var(--destructive);
  font-size: 13px;
}

@media (max-width: 960px) {
  .typed-object-header__hero--column,
  .typed-object-header__hero--inline {
    grid-template-columns: 1fr;
  }

  .typed-object-header__visual {
    max-width: 120px;
  }
}
</style>
