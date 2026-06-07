<script setup lang="ts">
import { shallowRef, toRef, watch } from "vue";
import ObjectTypeEditor from "./object-types/ObjectTypeEditor.vue";
import { useObjectTypeDraft } from "./object-types/useObjectTypeDraft";

const props = defineProps<{
  noteTypes: NoteType[];
  initialSelectedTypeId?: string | null;
  createDraftToken?: number;
  onNoteTypeSave: (
    draft: Omit<NoteType, "id" | "created_at" | "updated_at" | "slug"> & {
      id?: string;
      slug?: string;
    },
  ) => Promise<SaveNoteTypeResult | { ok: false }>;
  onNoteTypeDelete: (noteTypeId: string) => Promise<void>;
}>();

const emit = defineEmits<{
  selectedTypeChange: [typeId: string | null];
}>();

const {
  selectedTypeId,
  draft,
  typeError,
  isSystemDraft,
  previewNoteType,
  previewHeaderProps,
  previewEntries,
  openTypeEditor,
  closeTypeEditor,
  patchDraft,
  addField,
  removeField,
  moveField,
  moveFieldToIndex,
  updateField,
  buildNoteTypePayload,
} = useObjectTypeDraft(toRef(props, "noteTypes"));

const initializedCreateToken = shallowRef<number | null>(null);

watch(
  selectedTypeId,
  (nextTypeId) => {
    emit("selectedTypeChange", nextTypeId);
  },
  { immediate: true },
);

watch(
  () => props.initialSelectedTypeId,
  (nextTypeId) => {
    if (!nextTypeId) {
      return;
    }

    const targetType = props.noteTypes.find((noteType) => noteType.id === nextTypeId);
    if (!targetType) {
      return;
    }

    if (selectedTypeId.value === targetType.id && draft.value?.id === targetType.id) {
      return;
    }

    openTypeEditor(targetType);
  },
  { immediate: true },
);

watch(
  () => props.createDraftToken,
  (nextToken) => {
    if (nextToken == null) {
      return;
    }

    if (initializedCreateToken.value === null) {
      initializedCreateToken.value = nextToken;
      return;
    }

    if (nextToken === initializedCreateToken.value) {
      return;
    }

    initializedCreateToken.value = nextToken;
    openTypeEditor();
  },
  { immediate: true },
);

watch(
  () => props.noteTypes,
  (noteTypes) => {
    if (draft.value || noteTypes.length === 0) {
      return;
    }

    openTypeEditor(noteTypes[0]);
  },
  { immediate: true },
);

async function submitTypeEditor() {
  const noteTypePayload = buildNoteTypePayload();
  if (!noteTypePayload) {
    return;
  }

  const result = await props.onNoteTypeSave(noteTypePayload);
  if (!result.ok) {
    typeError.value =
      "message" in result ? (result as { message: string }).message : "Не удалось сохранить тип";
    return;
  }

  closeTypeEditor();
}

async function deleteType() {
  if (!draft.value?.id || isSystemDraft.value) {
    return;
  }

  await props.onNoteTypeDelete(draft.value.id);
  closeTypeEditor();
}
</script>

<template>
  <div class="object-types-scene">
    <ObjectTypeEditor
      :draft="draft"
      :is-system-draft="isSystemDraft"
      :type-error="typeError"
      :preview-note-type="previewNoteType"
      :preview-header-props="previewHeaderProps"
      :preview-entries="previewEntries"
      @close="closeTypeEditor"
      @save="submitTypeEditor"
      @delete="deleteType"
      @patch-draft="patchDraft"
      @add-field="addField"
      @remove-field="removeField"
      @move-field="moveField"
      @move-field-to-index="moveFieldToIndex"
      @update-field="updateField"
    />
  </div>
</template>

<style scoped>
.object-types-scene {
  min-width: 0;
  min-height: 100%;
  overflow-y: auto;
  background: var(--background);
}
</style>
