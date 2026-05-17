<script setup lang="ts">
import { computed } from "vue";
import ObjectTypeIdentitySection from "./ObjectTypeIdentitySection.vue";
import ObjectTypeFieldsSection from "./ObjectTypeFieldsSection.vue";
import ObjectTypePreviewRail from "./ObjectTypePreviewRail.vue";
import type { TypeDraft, TypeEditorFieldDraft } from "./shared";

const props = defineProps<{
  draft: TypeDraft | null;
  isSystemDraft: boolean;
  typeError: string | null;
  previewNoteType: NoteType | null;
  previewHeaderProps: Record<string, unknown>;
  previewEntries: Entry[];
}>();

const emit = defineEmits<{
  close: [];
  save: [];
  delete: [];
  patchDraft: [patch: Partial<TypeDraft>];
  addField: [];
  removeField: [fieldId: string];
  moveField: [index: number, direction: -1 | 1];
  moveFieldToIndex: [sourceIndex: number, targetIndex: number];
  updateField: [index: number, patch: Partial<TypeEditorFieldDraft>];
}>();

const draftSummary = computed(() => {
  if (!props.draft) {
    return null;
  }

  const featured = props.draft.fields.filter((field) => field.displayMode === "featured").length;
  const visible = props.draft.fields.filter((field) => field.displayMode === "visible").length;
  const hidden = props.draft.fields.filter((field) => field.displayMode === "hidden").length;

  return {
    total: props.draft.fields.length,
    featured,
    visible,
    hidden,
  };
});

function handleMoveField(index: number, direction: -1 | 1) {
  emit("moveField", index, direction);
}

function handleMoveFieldToIndex(sourceIndex: number, targetIndex: number) {
  emit("moveFieldToIndex", sourceIndex, targetIndex);
}

function handleUpdateField(index: number, patch: Partial<TypeEditorFieldDraft>) {
  emit("updateField", index, patch);
}
</script>

<template>
  <section v-if="draft" class="object-type-editor">
    <header class="object-type-editor__head">
      <div class="object-type-editor__title-block">
        <div class="object-type-editor__eyebrow">
          {{ draft.id ? "Редактирование типа" : "Новый тип" }}
        </div>
        <h2 class="object-type-editor__title">{{ draft.name || "Новый тип объекта" }}</h2>
        <p class="object-type-editor__subtitle">
          Тип задает только верхнюю часть объекта. Тело заметки остается одинаковым для всех
          объектов, а здесь настраиваются свойства, их видимость и подача в шапке.
        </p>

        <div v-if="draftSummary" class="object-type-editor__stats">
          <span class="object-type-editor__stat-pill">{{ draftSummary.total }} полей</span>
          <span class="object-type-editor__stat-pill">{{ draftSummary.featured }} в шапке</span>
          <span class="object-type-editor__stat-pill">{{ draftSummary.visible }} в свойствах</span>
          <span class="object-type-editor__stat-pill">{{ draftSummary.hidden }} скрыто</span>
        </div>
      </div>

      <div class="object-type-editor__actions">
        <button class="object-type-editor__ghost" type="button" @click="emit('close')">
          Отмена
        </button>
        <button class="object-type-editor__primary" type="button" @click="emit('save')">
          Сохранить тип
        </button>
      </div>
    </header>

    <div class="object-type-editor__body">
      <div class="object-type-editor__main">
        <ObjectTypeIdentitySection
          :draft="draft"
          :is-system-draft="isSystemDraft"
          @patch-draft="emit('patchDraft', $event)"
        />

        <ObjectTypeFieldsSection
          :draft="draft"
          :is-system-draft="isSystemDraft"
          @add-field="emit('addField')"
          @remove-field="emit('removeField', $event)"
          @move-field="handleMoveField"
          @move-field-to-index="handleMoveFieldToIndex"
          @update-field="handleUpdateField"
        />

        <div v-if="typeError" class="object-type-editor__error">
          {{ typeError }}
        </div>

        <div v-if="!isSystemDraft && draft.id" class="object-type-editor__delete-row">
          <button class="object-type-editor__danger" type="button" @click="emit('delete')">
            Удалить тип
          </button>
        </div>
      </div>

      <ObjectTypePreviewRail
        :preview-note-type="previewNoteType"
        :preview-header-props="previewHeaderProps"
        :preview-entries="previewEntries"
        :title="draft.name || 'Новый объект'"
      />
    </div>
  </section>

  <div v-else class="object-type-editor-empty">
    <div class="object-type-editor-empty__eyebrow">Библиотека типов</div>
    <div class="object-type-editor-empty__title">Выберите тип объекта или создайте новый</div>
    <p class="object-type-editor-empty__text">
      Слева находится библиотека типов, справа появится полноценный редактор структуры, свойств и
      визуальной подачи объекта.
    </p>
  </div>
</template>

<style scoped>
.object-type-editor,
.object-type-editor-empty {
  min-width: 0;
  min-height: 100%;
}

.object-type-editor {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
}

.object-type-editor__head {
  position: sticky;
  top: 0;
  z-index: 4;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  padding: 28px 32px 22px;
  border-bottom: 1px solid var(--border);
  background: color-mix(in srgb, var(--background) 92%, var(--surface));
  backdrop-filter: blur(18px);
}

.object-type-editor__title-block {
  max-width: 760px;
}

.object-type-editor__eyebrow,
.object-type-editor-empty__eyebrow {
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.object-type-editor__title,
.object-type-editor-empty__title {
  margin: 8px 0 0;
  color: var(--foreground);
  font-size: var(--kosmos-text-page-title-size);
  line-height: var(--kosmos-text-page-title-line-height);
  font-weight: var(--kosmos-text-page-title-weight);
  letter-spacing: var(--kosmos-text-page-title-letter-spacing);
}

.object-type-editor__subtitle {
  margin: 10px 0 0;
  color: var(--muted-foreground);
  font-size: 14px;
  line-height: 1.62;
}

.object-type-editor__stats {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 14px;
}

.object-type-editor__stat-pill {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--background);
  color: var(--muted-foreground);
  font-size: 12px;
}

.object-type-editor__actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.object-type-editor__body {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(320px, 380px);
  gap: 28px;
  padding: 24px 32px 36px;
}

.object-type-editor__main {
  display: grid;
  gap: 18px;
  min-width: 0;
}

.object-type-editor__ghost,
.object-type-editor__primary,
.object-type-editor__danger {
  min-height: 36px;
  padding: 0 14px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
}

.object-type-editor__ghost {
  border: 1px solid var(--border);
  background: var(--secondary);
  color: var(--secondary-foreground);
}

.object-type-editor__primary {
  border: 1px solid var(--primary);
  background: var(--primary);
  color: var(--primary-foreground);
}

.object-type-editor__danger {
  border: 1px solid var(--destructive);
  background: transparent;
  color: var(--destructive);
}

.object-type-editor__error {
  padding: 12px 14px;
  border: 1px solid var(--destructive);
  border-radius: 14px;
  background: var(--background);
  color: var(--destructive);
  font-size: 13px;
}

.object-type-editor__delete-row {
  display: flex;
  justify-content: flex-start;
}

.object-type-editor-empty {
  display: grid;
  place-content: center;
  gap: 10px;
  padding: 48px;
}

.object-type-editor-empty__text {
  max-width: 480px;
  margin: 0;
  color: var(--muted-foreground);
  font-size: 14px;
  line-height: 1.6;
}

@media (max-width: 1280px) {
  .object-type-editor__body {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 960px) {
  .object-type-editor__head {
    flex-direction: column;
    padding: 24px 20px 18px;
  }

  .object-type-editor__body {
    padding: 20px;
  }
}
</style>
