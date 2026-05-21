<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import type { TypeDraft } from "./shared";
import { COLOR_OPTIONS, ICON_OPTIONS } from "./shared";
import { objectIconUri } from "@/lib/iconResolver";

const props = defineProps<{
  draft: TypeDraft;
  isSystemDraft: boolean;
}>();

const emit = defineEmits<{
  patchDraft: [patch: Partial<TypeDraft>];
}>();

const pickerOpen = ref(false);
const pickerRoot = ref<HTMLElement | null>(null);

const iconSrc = computed(() => objectIconUri(props.draft.icon));

function updateDraftValue<K extends keyof TypeDraft>(key: K, value: TypeDraft[K]) {
  emit("patchDraft", { [key]: value });
}

function updateDraftString(key: keyof TypeDraft, event: Event) {
  updateDraftValue(
    key,
    (event.target as HTMLInputElement | HTMLSelectElement).value as TypeDraft[keyof TypeDraft],
  );
}

function togglePicker() {
  if (props.isSystemDraft) {
    return;
  }

  pickerOpen.value = !pickerOpen.value;
}

function selectIcon(icon: string) {
  updateDraftValue("icon", icon);
}

function selectColor(color: string) {
  updateDraftValue("color", color);
}

function handlePointerDown(event: MouseEvent) {
  if (!pickerOpen.value) {
    return;
  }

  const target = event.target;
  if (!(target instanceof Node)) {
    return;
  }

  if (pickerRoot.value?.contains(target)) {
    return;
  }

  pickerOpen.value = false;
}

if (typeof window !== "undefined") {
  window.addEventListener("pointerdown", handlePointerDown);
}

onBeforeUnmount(() => {
  if (typeof window !== "undefined") {
    window.removeEventListener("pointerdown", handlePointerDown);
  }
});
</script>

<template>
  <section class="object-type-section object-type-section--identity">
    <div class="object-type-section__head">
      <div class="object-type-section__heading">
        <div class="object-type-section__eyebrow">Основа</div>
        <h3 class="object-type-section__title">Идентичность типа</h3>
        <p class="object-type-section__text">
          Название, множественное имя и визуальная подача определяют, как тип выглядит в библиотеке
          и на странице коллекции. Заголовок заметки остается отдельным top-level полем, а свойства
          ниже управляют верхней частью страницы.
        </p>
      </div>

      <span v-if="isSystemDraft" class="object-type-section__badge type-editor-chip">
        Контракт защищен кодом
      </span>
    </div>

    <div class="object-type-identity__hero">
      <div ref="pickerRoot" class="object-type-identity__picker">
        <button
          type="button"
          class="object-type-identity__picker-trigger"
          :style="{
            '--object-type-accent': draft.color || '#2aa7ee',
            '--object-type-icon-src': `url(${iconSrc})`,
          }"
          :disabled="isSystemDraft"
          :aria-expanded="pickerOpen ? 'true' : 'false'"
          @click="togglePicker"
        >
          <span class="object-type-identity__icon" aria-hidden="true" />
        </button>

        <div v-if="pickerOpen" class="object-type-identity__picker-panel">
          <div class="object-type-identity__picker-section">
            <div class="object-type-identity__picker-label">Иконка</div>
            <div class="object-type-identity__icon-grid">
              <button
                v-for="icon in ICON_OPTIONS"
                :key="icon"
                type="button"
                class="object-type-identity__icon-option"
                :class="{ 'is-active': draft.icon === icon }"
                :title="icon"
                :aria-label="icon"
                :style="{
                  '--object-type-accent': draft.color || '#2aa7ee',
                  '--object-type-icon-src': `url(${objectIconUri(icon)})`,
                }"
                @click="selectIcon(icon)"
              >
                <span class="object-type-identity__icon-option-glyph" aria-hidden="true" />
              </button>
            </div>
          </div>

          <div class="object-type-identity__picker-section">
            <div class="object-type-identity__picker-label">Цвет</div>
            <div class="object-type-identity__color-grid">
              <button
                v-for="color in COLOR_OPTIONS"
                :key="color"
                type="button"
                class="object-type-identity__color-option"
                :class="{ 'is-active': draft.color === color }"
                :title="color"
                :aria-label="`Цвет ${color}`"
                :style="{ '--object-type-accent': color }"
                @click="selectColor(color)"
              >
                <span class="object-type-identity__color-option-dot" aria-hidden="true" />
              </button>
            </div>
          </div>
        </div>
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

      <label class="object-type-form-field object-type-form-field--wide">
        <span>Визуал типа</span>
        <button
          type="button"
          class="object-type-identity__visual-trigger"
          :disabled="isSystemDraft"
          @click="togglePicker"
        >
          <span
            class="object-type-identity__visual-icon"
            :style="{
              '--object-type-accent': draft.color || '#2aa7ee',
              '--object-type-icon-src': `url(${iconSrc})`,
            }"
            aria-hidden="true"
          />
          <span class="object-type-identity__visual-copy">
            Иконка: {{ draft.icon }} · Цвет: {{ draft.color }}
          </span>
        </button>
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

.object-type-identity__picker {
  position: relative;
  flex-shrink: 0;
}

.object-type-identity__picker-trigger {
  display: inline-grid;
  place-items: center;
  width: 72px;
  height: 72px;
  border: none;
  border-radius: 18px;
  background: transparent;
  cursor: pointer;
}

.object-type-identity__picker-trigger:disabled,
.object-type-identity__visual-trigger:disabled {
  cursor: default;
  opacity: 0.6;
}

.object-type-identity__icon,
.object-type-identity__visual-icon,
.object-type-identity__icon-option-glyph {
  display: inline-block;
  background-color: var(--object-type-accent);
  mask-image: var(--object-type-icon-src);
  mask-repeat: no-repeat;
  mask-position: center;
  mask-size: contain;
  -webkit-mask-image: var(--object-type-icon-src);
  -webkit-mask-repeat: no-repeat;
  -webkit-mask-position: center;
  -webkit-mask-size: contain;
}

.object-type-identity__icon {
  width: 40px;
  height: 40px;
}

.object-type-identity__picker-panel {
  position: absolute;
  top: calc(100% + 10px);
  left: 0;
  z-index: 10;
  display: grid;
  gap: 16px;
  min-width: 320px;
  padding: 16px;
  border: 1px solid var(--border);
  border-radius: 18px;
  background: var(--popover);
  box-shadow: 0 20px 40px rgb(0 0 0 / 0.24);
}

.object-type-identity__picker-section {
  display: grid;
  gap: 10px;
}

.object-type-identity__picker-label {
  color: var(--muted-foreground);
  font-size: 12px;
  font-weight: 600;
}

.object-type-identity__icon-grid,
.object-type-identity__color-grid {
  display: grid;
  gap: 8px;
}

.object-type-identity__icon-grid {
  grid-template-columns: repeat(5, minmax(0, 1fr));
}

.object-type-identity__color-grid {
  grid-template-columns: repeat(5, minmax(0, 1fr));
}

.object-type-identity__icon-option,
.object-type-identity__color-option {
  display: inline-grid;
  place-items: center;
  height: 44px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--background);
  transition:
    border-color 0.16s ease,
    background-color 0.16s ease;
}

.object-type-identity__icon-option.is-active,
.object-type-identity__color-option.is-active {
  border-color: color-mix(in srgb, var(--object-type-accent) 72%, var(--ring));
  background: color-mix(in srgb, var(--object-type-accent) 10%, transparent);
}

.object-type-identity__icon-option-glyph {
  width: 22px;
  height: 22px;
}

.object-type-identity__color-option-dot {
  width: 20px;
  height: 20px;
  border-radius: 999px;
  background: var(--object-type-accent);
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

.object-type-identity__visual-trigger {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  min-height: 44px;
  padding: 0 12px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--background);
  color: var(--foreground);
  text-align: left;
}

.object-type-identity__visual-icon {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
}

.object-type-identity__visual-copy {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 720px) {
  .object-type-identity__hero {
    align-items: flex-start;
    flex-direction: column;
  }

  .object-type-identity__picker-panel {
    min-width: min(320px, calc(100vw - 64px));
  }
}
</style>
