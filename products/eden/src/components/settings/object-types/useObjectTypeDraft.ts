import { computed, ref, type Ref } from "vue";
import { isSystemType } from "@/lib/systemTypes";
import { createDefaultHeaderTemplate } from "@/lib/typedNotes";
import type { TypeDraft, TypeEditorFieldDraft } from "./shared";
import {
  buildHeaderTemplateJson,
  buildUiSchema,
  createEmptyTypeDraft,
  createTypeDraftFromNoteType,
  sampleValueForField,
  toSchemaFields,
} from "./shared";

function normalizedIncludes(value: string, query: string) {
  return value.toLowerCase().includes(query.toLowerCase());
}

export function useObjectTypeDraft(noteTypes: Ref<NoteType[]>) {
  const selectedTypeId = ref<string | null>(null);
  const draft = ref<TypeDraft | null>(null);
  const typeError = ref<string | null>(null);
  const filterQuery = ref("");

  const filteredTypes = computed(() => {
    const query = filterQuery.value.trim();
    if (!query) {
      return noteTypes.value;
    }

    return noteTypes.value.filter(
      (noteType) =>
        normalizedIncludes(noteType.name, query) || normalizedIncludes(noteType.id, query),
    );
  });

  const builtInTypes = computed(() =>
    filteredTypes.value.filter((noteType) => isSystemType(noteType.id)),
  );

  const customTypes = computed(() =>
    filteredTypes.value.filter((noteType) => !isSystemType(noteType.id)),
  );

  const isSystemDraft = computed(() => (draft.value?.id ? isSystemType(draft.value.id) : false));

  const previewNoteType = computed<NoteType | null>(() => {
    if (!draft.value) {
      return null;
    }

    return {
      id: draft.value.id ?? "preview-type",
      name: draft.value.name || "Новый тип",
      slug: draft.value.slug || "preview-type",
      icon: draft.value.icon,
      color: draft.value.color,
      schema_json: JSON.stringify({ fields: toSchemaFields(draft.value.fields) }),
      header_template_json:
        draft.value.fields.length > 0
          ? buildHeaderTemplateJson(draft.value)
          : JSON.stringify(createDefaultHeaderTemplate("default")),
      ui_schema_json: JSON.stringify(buildUiSchema(draft.value)),
      created_at: 0,
      updated_at: 0,
    };
  });

  const previewHeaderProps = computed(() => {
    if (!draft.value) {
      return {};
    }

    return Object.fromEntries(
      draft.value.fields.map((field) => {
        switch (field.kind) {
          case "boolean":
            return [field.id, true];
          case "number":
            return [field.id, field.id === "user_rating" ? "8" : "120"];
          case "date":
            return [field.id, "2026-04-20"];
          case "select":
            return [field.id, field.options?.[0] ?? ""];
          case "multi_select":
            return [field.id, field.options?.slice(0, 2) ?? []];
          case "relation":
            return [field.id, ["preview-related", "preview-reference"]];
          case "image":
            return [
              field.id,
              field.id === "background_image"
                ? "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1200 320'><rect width='1200' height='320' fill='%2315181f'/><circle cx='180' cy='160' r='120' fill='%23263044'/><rect x='340' y='54' width='720' height='212' rx='28' fill='%23252b36'/></svg>"
                : "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 320 320'><rect width='320' height='320' rx='42' fill='%23151a22'/><rect x='34' y='34' width='252' height='252' rx='28' fill='%2328364a'/></svg>",
            ];
          default:
            return [field.id, sampleValueForField(field)];
        }
      }),
    );
  });

  const previewEntries = computed<Entry[]>(() => [
    {
      id: "preview-related",
      title: "Связанная заметка",
      content_json: "{}",
      created_at: 0,
      updated_at: 0,
      folder_id: null,
      type_id: "note_obj",
      header_layout: "inline",
      header_props_json: "{}",
      schema_version: 1,
      deleted_at: null,
    },
    {
      id: "preview-reference",
      title: "Референс",
      content_json: "{}",
      created_at: 0,
      updated_at: 0,
      folder_id: null,
      type_id: "note_obj",
      header_layout: "inline",
      header_props_json: "{}",
      schema_version: 1,
      deleted_at: null,
    },
  ]);

  function openTypeEditor(noteType?: NoteType) {
    draft.value = noteType ? createTypeDraftFromNoteType(noteType) : createEmptyTypeDraft();
    selectedTypeId.value = noteType?.id ?? null;
    typeError.value = null;
  }

  function closeTypeEditor() {
    draft.value = null;
    selectedTypeId.value = null;
    typeError.value = null;
  }

  function patchDraft(patch: Partial<TypeDraft>) {
    if (!draft.value) {
      return;
    }

    draft.value = {
      ...draft.value,
      ...patch,
    };
  }

  function addField() {
    if (!draft.value || isSystemDraft.value) {
      return;
    }

    draft.value.fields.push({
      id: `field_${draft.value.fields.length + 1}`,
      label: "Новое поле",
      kind: "text",
      required: false,
      read_only: false,
      visible: true,
      displayMode: "visible",
      structuralLocked: false,
    });
  }

  function removeField(fieldId: string) {
    if (!draft.value || isSystemDraft.value) {
      return;
    }

    draft.value.fields = draft.value.fields.filter((field) => field.id !== fieldId);
  }

  function moveField(index: number, direction: -1 | 1) {
    moveFieldToIndex(index, index + direction);
  }

  function moveFieldToIndex(sourceIndex: number, targetIndex: number) {
    if (!draft.value) {
      return;
    }

    if (
      sourceIndex < 0 ||
      sourceIndex >= draft.value.fields.length ||
      targetIndex < 0 ||
      targetIndex >= draft.value.fields.length ||
      sourceIndex === targetIndex
    ) {
      return;
    }

    const nextFields = [...draft.value.fields];
    const [field] = nextFields.splice(sourceIndex, 1);
    if (!field) {
      return;
    }

    nextFields.splice(targetIndex, 0, field);
    draft.value.fields = nextFields;
  }

  function updateField(index: number, patch: Partial<TypeEditorFieldDraft>) {
    if (!draft.value) {
      return;
    }

    draft.value.fields = draft.value.fields.map((field, currentIndex) =>
      currentIndex === index ? { ...field, ...patch } : field,
    );
  }

  function buildNoteTypePayload() {
    if (!draft.value) {
      return null;
    }

    return {
      id: draft.value.id,
      name: draft.value.name.trim(),
      slug: draft.value.slug.trim() || draft.value.name.trim(),
      icon: draft.value.icon || "document",
      color: draft.value.color || "#2aa7ee",
      schema_json: JSON.stringify({ fields: toSchemaFields(draft.value.fields) }),
      header_template_json: buildHeaderTemplateJson(draft.value),
      ui_schema_json: JSON.stringify(buildUiSchema(draft.value)),
    };
  }

  return {
    selectedTypeId,
    draft,
    typeError,
    filterQuery,
    builtInTypes,
    customTypes,
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
  };
}
