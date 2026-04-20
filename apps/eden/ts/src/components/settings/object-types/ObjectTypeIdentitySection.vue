<script setup lang="ts">
import { computed } from "vue";
import type { TypeDraft } from "./shared";
import { ICON_OPTIONS } from "./shared";

const props = defineProps<{
  draft: TypeDraft;
  isSystemDraft: boolean;
}>();

const emit = defineEmits<{
  patchDraft: [patch: Partial<TypeDraft>];
}>();

const iconSrc = computed(() => `/anytype/icon/type/default/${props.draft.icon || "document"}.svg`);

function updateDraftString(key: keyof TypeDraft, event: Event) {
  emit("patchDraft", { [key]: (event.target as HTMLInputElement | HTMLSelectElement).value });
}
</script>

<template>
  <section class="object-type-section object-type-section--identity">
    <div class="object-type-section__head">
      <div class="object-type-section__heading">
        <div class="object-type-section__eyebrow">Основа</div>
        <h3 class="object-type-section__title">Идентичность типа</h3>
        <p class="object-type-section__text">
          Название, множественное имя и визуальная подача определяют, как тип выглядит в библиотеке и
          на странице коллекции. Заголовок заметки остается отдельным top-level полем, а свойства ниже
          управляют верхней частью страницы.
        </p>
      </div>

      <span v-if="isSystemDraft" class="object-type-section__badge type-editor-chip">
        Контракт защищён кодом
      </span>
    </div>

    <div class="object-type-identity__hero">
      <div
        class="object-type-identity__icon-tile"
        :style="{ '--object-type-accent': draft.color || '#2aa7ee' }"
      >
        <img class="object-type-identity__icon" :src="iconSrc" alt="" width="34" height="34" draggable="false" />
      </div>

      <div class="object-type-identity__hero-copy">
        <div class="object-type-identity__hero-label">Как тип будет выглядеть в библиотеке</div>
        <div class="object-type-identity__hero-title">{{ draft.name || "Новый тип объекта" }}</div>
        <div class="object-type-identity__hero-meta">
          {{ draft.collectionName || draft.name || "Объекты" }} · {{ draft.slug || "note-type" }}
        </div>
      </div>
    </div>

    <div class="object-type-form-grid">
      <label class="object-type-form-field object-type-form-field--wide">
        <span>Название</span>
        <input
          class="object-type-form-input object-type-form-input--large"
          :value="draft.name"
          :disabled="isSystemDraft"
          placeholder="Например, Книга"
          @input="updateDraftString('name', $event)"
        />
      </label>

      <label class="object-type-form-field object-type-form-field--wide">
        <span>Множественное имя</span>
        <input
          class="object-type-form-input"
          :value="draft.collectionName"
          :disabled="isSystemDraft"
          placeholder="Например, Книги"
          @input="updateDraftString('collectionName', $event)"
        />
      </label>

      <label class="object-type-form-field">
        <span>Идентификатор</span>
        <input
          class="object-type-form-input object-type-form-input--mono"
          :value="draft.slug"
          :disabled="isSystemDraft"
          placeholder="book_obj"
          @input="updateDraftString('slug', $event)"
        />
      </label>

      <label class="object-type-form-field">
        <span>Иконка</span>
        <select
          class="object-type-form-input"
          :value="draft.icon"
          @change="updateDraftString('icon', $event)"
        >
          <option v-for="icon in ICON_OPTIONS" :key="icon" :value="icon">
            {{ icon }}
          </option>
        </select>
      </label>

      <label class="object-type-form-field">
        <span>Цвет акцента</span>
        <div class="object-type-form-color">
          <input
            class="object-type-form-color-picker"
            :value="draft.color"
            type="color"
            @input="updateDraftString('color', $event)"
          />
          <input
            class="object-type-form-input object-type-form-input--mono"
            :value="draft.color"
            @input="updateDraftString('color', $event)"
          />
        </div>
      </label>

      <label class="object-type-form-field">
        <span>Макет шапки</span>
        <select
          class="object-type-form-input"
          :value="draft.headerLayout"
          @change="updateDraftString('headerLayout', $event)"
        >
          <option value="inline">Inline</option>
          <option value="column">Column</option>
        </select>
      </label>
    </div>
  </section>
</template>

<style scoped>
.object-type-section {
  display: grid;
  gap: 18px;
  padding: 22px 24px;
  border: 1px solid var(--border);
  border-radius: 20px;
  background: var(--surface);
}

.object-type-section__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.object-type-section__heading {
  max-width: 720px;
}

.object-type-section__eyebrow {
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.object-type-section__title {
  margin: 6px 0 0;
  color: var(--foreground);
  font-size: 22px;
  line-height: 1.08;
  font-weight: 600;
  letter-spacing: -0.03em;
}

.object-type-section__text {
  margin: 10px 0 0;
  color: var(--muted-foreground);
  font-size: 14px;
  line-height: 1.6;
}

.object-type-section__badge {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--background);
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
}

.object-type-identity__hero {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 14px 16px;
  border: 1px solid var(--border);
  border-radius: 18px;
  background: var(--background);
}

.object-type-identity__icon-tile {
  display: grid;
  place-items: center;
  width: 72px;
  height: 72px;
  border-radius: 20px;
  background:
    linear-gradient(
      180deg,
      color-mix(in srgb, var(--object-type-accent) 28%, transparent),
      color-mix(in srgb, var(--object-type-accent) 12%, transparent)
    ),
    var(--surface);
  border: 1px solid color-mix(in srgb, var(--object-type-accent) 32%, var(--border));
}

.object-type-identity__icon {
  filter: invert(1);
  opacity: 0.88;
}

.object-type-identity__hero-copy {
  display: grid;
  gap: 4px;
}

.object-type-identity__hero-label {
  color: var(--muted-foreground);
  font-size: 12px;
  line-height: 1.4;
}

.object-type-identity__hero-title {
  color: var(--foreground);
  font-size: 20px;
  line-height: 1.15;
  font-weight: 600;
}

.object-type-identity__hero-meta {
  color: var(--muted-foreground);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
}

.object-type-form-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 14px 16px;
}

.object-type-form-field {
  display: grid;
  gap: 8px;
}

.object-type-form-field--wide {
  grid-column: 1 / -1;
}

.object-type-form-field > span {
  color: var(--muted-foreground);
  font-size: 12px;
  font-weight: 500;
}

.object-type-form-input {
  width: 100%;
  min-height: 40px;
  padding: 0 12px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--background);
  color: var(--foreground);
}

.object-type-form-input--large {
  min-height: 46px;
  font-size: 16px;
}

.object-type-form-input--mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

.object-type-form-color {
  display: grid;
  grid-template-columns: 44px 1fr;
  gap: 10px;
}

.object-type-form-color-picker {
  width: 44px;
  min-width: 44px;
  height: 40px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--background);
}
</style>
