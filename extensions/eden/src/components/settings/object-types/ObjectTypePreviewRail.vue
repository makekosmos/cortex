<script setup lang="ts">
import TypedHeader from "@/components/typed-notes/TypedHeader.vue";

defineProps<{
  previewNoteType: NoteType | null;
  previewHeaderProps: Record<string, unknown>;
  previewEntries: Entry[];
  title: string;
}>();
</script>

<template>
  <aside class="object-type-preview-rail" data-testid="object-type-preview-rail">
    <div class="object-type-preview-rail__sticky">
      <div class="object-type-preview-rail__eyebrow">Превью</div>
      <h3 class="object-type-preview-rail__title">Как это будет ощущаться в Eden</h3>
      <p class="object-type-preview-rail__text">
        Справа находится не форма, а живая шапка объекта: тип, описание, свойства из шапки и
        обычные свойства читаются так же, как они будут выглядеть в самой заметке.
      </p>

      <div class="object-type-preview-rail__surface">
        <TypedHeader
          :active-note-type="previewNoteType"
          :title="title"
          :header-props="previewHeaderProps"
          :validation-error="null"
          :all-entries="previewEntries"
          current-entry-id="preview-entry"
          readonly
          show-title
          @header-prop-change="() => undefined"
          @relation-navigate="() => undefined"
        />
      </div>
    </div>
  </aside>
</template>

<style scoped>
.object-type-preview-rail {
  min-width: 0;
  padding-left: 8px;
  border-left: 1px solid var(--border);
}

.object-type-preview-rail__sticky {
  position: sticky;
  top: 24px;
  display: grid;
  gap: 12px;
}

.object-type-preview-rail__eyebrow {
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.object-type-preview-rail__title {
  margin: 0;
  color: var(--foreground);
  font-size: 20px;
  line-height: 1.12;
  font-weight: 600;
  letter-spacing: -0.02em;
}

.object-type-preview-rail__text {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 13px;
  line-height: 1.55;
}

.object-type-preview-rail__surface {
  padding: 18px 18px 20px;
  border: 1px solid var(--border);
  border-radius: 22px;
  background: var(--card);
  box-shadow: 0 18px 32px rgb(0 0 0 / 0.08);
}

@media (max-width: 1200px) {
  .object-type-preview-rail {
    padding-left: 0;
    border-left: none;
  }

  .object-type-preview-rail__sticky {
    position: static;
  }
}
</style>
