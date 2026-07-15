<script setup lang="ts">
import { computed } from "vue";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { formatObjectFieldValue } from "@/lib/objectFieldFormatting";
import { resolveNoteTypeFields, type ResolvedNoteTypeField } from "@/lib/typedNotes";
import { safeParseHeaderProps } from "@/lib/typedNoteHeaderProps";
import { toDisplayImageSrc } from "@/lib/localImages";
import { readEntryMarkdown } from "@/editor-content/content";

const props = defineProps<{
  entry: Entry;
  noteType: NoteType;
}>();

const headerProps = computed(() =>
  safeParseHeaderProps(props.noteType, props.entry.header_props_json),
);
const titleText = computed(() =>
  getEntryDisplayTitle(props.entry.title, props.entry.header_props_json),
);
function firstMarkdownImageSrc(markdown: string): string {
  const inline = /!\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/.exec(markdown);
  if (inline?.[1]) return inline[1].trim();

  const wikilink = /!\[\[([^|\]]+)(?:\|[^\]]+)?\]\]/.exec(markdown);
  return wikilink?.[1]?.trim() ?? "";
}

const imageSrc = computed(() => {
  const image = String(headerProps.value.image ?? "").trim();
  const sourcePath = String(headerProps.value.source_path ?? "").trim();
  const bodyImage = firstMarkdownImageSrc(readEntryMarkdown(props.entry.content_json));
  const imageLooksResolvable =
    /^[a-z][a-z0-9+.-]*:/i.test(image) || /^[A-Za-z]:[\\/]/.test(image) || image.startsWith("\\\\");
  return toDisplayImageSrc(imageLooksResolvable ? image : sourcePath || bodyImage || image);
});
const altText = computed(() => String(headerProps.value.alt_text ?? titleText.value).trim());
const detailFields = computed(() =>
  resolveNoteTypeFields(props.noteType).filter(
    (field) =>
      field.visible && field.id !== "image" && hasFieldValue(field, headerProps.value[field.id]),
  ),
);

function hasFieldValue(field: ResolvedNoteTypeField, value: unknown): boolean {
  return formatObjectFieldValue(field, value).trim().length > 0;
}

function fieldValue(field: ResolvedNoteTypeField): string {
  return formatObjectFieldValue(field, headerProps.value[field.id]);
}
</script>

<template>
  <section class="image-object-view kosmos-scroll" data-testid="image-object-view">
    <div class="image-object-shell">
      <figure class="image-object-preview">
        <img
          v-if="imageSrc"
          class="image-object-preview__image"
          :src="imageSrc"
          :alt="altText"
          draggable="false"
        />
        <div v-else class="image-object-preview__empty">Изображение не найдено</div>
      </figure>

      <aside class="image-object-details" aria-label="Свойства изображения">
        <dl v-if="detailFields.length > 0" class="image-object-fields kosmos-scroll">
          <div v-for="field in detailFields" :key="field.id" class="image-object-field">
            <dt class="image-object-field__label">{{ field.label }}</dt>
            <dd class="image-object-field__value">{{ fieldValue(field) }}</dd>
          </div>
        </dl>
      </aside>
    </div>
  </section>
</template>

<style scoped>
.image-object-view {
  height: 100%;
  min-height: 100%;
  padding: 80px 40px 40px;
  overflow-y: auto;
  overflow-x: hidden;
}

.image-object-shell {
  display: grid;
  margin: 0 auto;
  width: min(100%, 920px);
  grid-template-rows: auto auto;
  gap: 22px;
}

.image-object-preview {
  display: grid;
  width: 100%;
  min-width: 0;
  min-height: 0;
  margin: 0;
  place-items: center;
  overflow: visible;
}

.image-object-preview__image {
  display: block;
  max-width: 100%;
  max-height: min(70vh, 720px);
  height: auto;
  object-fit: contain;
  border-radius: 8px;
}

.image-object-preview__empty {
  color: var(--muted-foreground);
  font-size: 14px;
}

.image-object-details {
  min-width: 0;
}

.image-object-fields {
  display: grid;
  gap: 0;
  margin: 0;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  padding: 2px 0 0;
}

.image-object-field {
  display: grid;
  grid-template-columns: 160px minmax(0, 1fr);
  gap: 12px;
  border-bottom: 1px solid color-mix(in srgb, var(--foreground) 7%, transparent);
  padding: 9px 0;
}

.image-object-field__label,
.image-object-field__value {
  margin: 0;
}

.image-object-field__label {
  color: color-mix(in srgb, var(--foreground) 42%, transparent);
  font-size: 12px;
  font-weight: 650;
}

.image-object-field__value {
  color: var(--foreground);
  font-size: 12px;
  line-height: 1.4;
  overflow-wrap: anywhere;
  text-align: right;
}

@media (max-width: 920px) {
  .image-object-view {
    padding: 80px 24px 32px;
  }

  .image-object-shell {
    width: 100%;
  }

  .image-object-field {
    grid-template-columns: 120px minmax(0, 1fr);
  }
}
</style>
