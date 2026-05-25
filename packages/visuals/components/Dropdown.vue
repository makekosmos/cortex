<script setup lang="ts" generic="T extends string | number">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { ChevronDown, Check } from "@lucide/vue";

/**
 * Универсальный Dropdown (shadcn-стиль): кастомный триггер + popover с
 * опциями, teleport в body. v-model по значению (`update:modelValue`).
 * Закрывается по клику снаружи, Escape, выбору.
 */

interface Option<V> {
  value: V;
  label: string;
  /** Опциональная подсказка под лейблом. */
  description?: string;
  /** Disable конкретной опции. */
  disabled?: boolean;
}

interface Props<V> {
  modelValue: V | null;
  options: Option<V>[];
  placeholder?: string;
  /** Если true — pop-up равен ширине триггера. По умолчанию true. */
  matchTriggerWidth?: boolean;
  /** Disable весь триггер. */
  disabled?: boolean;
  /** Показывать ли search field в popup'е. По умолчанию — auto: search
   * появляется когда опций ≥ 6 (для коротких списков он избыточен). */
  searchable?: boolean | "auto";
  /** Placeholder для search input'а. */
  searchPlaceholder?: string;
}

const props = withDefaults(defineProps<Props<T>>(), {
  placeholder: "Выбрать…",
  matchTriggerWidth: true,
  disabled: false,
  searchable: "auto",
  searchPlaceholder: "Поиск…",
});

const emit = defineEmits<{
  "update:modelValue": [v: T];
}>();

const open = ref(false);
const triggerRef = ref<HTMLElement | null>(null);
const panelRef = ref<HTMLElement | null>(null);
const searchInputRef = ref<HTMLInputElement | null>(null);
const searchQuery = ref("");
const panelPosition = ref<{ top: number; left: number; width: number }>({
  top: 0,
  left: 0,
  width: 0,
});
const highlightIdx = ref(0);

const isSearchable = computed(() => {
  if (props.searchable === "auto") return props.options.length >= 6;
  return !!props.searchable;
});

const filteredOptions = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return props.options;
  return props.options.filter(
    (o) =>
      o.label.toLowerCase().includes(q) ||
      (o.description && o.description.toLowerCase().includes(q)),
  );
});

const selectedOption = computed<Option<T> | null>(() => {
  if (props.modelValue === null || props.modelValue === undefined) return null;
  return props.options.find((o) => o.value === props.modelValue) ?? null;
});

const displayLabel = computed(() =>
  selectedOption.value ? selectedOption.value.label : props.placeholder,
);

function reposition() {
  const trigger = triggerRef.value;
  const panel = panelRef.value;
  if (!trigger || !panel) return;
  const rect = trigger.getBoundingClientRect();
  const panelH = panel.offsetHeight;
  const panelW = panel.offsetWidth;
  const margin = 4;
  const edgePad = 8;

  // Vertical: выбираем сторону с большим пространством.
  const spaceBelow = window.innerHeight - rect.bottom - margin;
  const spaceAbove = rect.top - margin;
  let top: number;
  if (spaceBelow >= spaceAbove) {
    top = rect.bottom + margin;
  } else {
    top = Math.max(rect.top - panelH - margin, edgePad);
  }

  // Horizontal: по умолчанию anchor по ПРАВОМУ краю trigger'а (panel
  // расходится влево). Это естественно когда trigger — узкая кнопка справа
  // в строке settings (язык, провайдер). Если левый край panel'а уходит за
  // viewport — flip в left-anchor.
  let left = rect.right - panelW;
  if (left < edgePad) {
    left = rect.left;
  }
  // Final clamp в обе стороны viewport.
  left = Math.max(edgePad, Math.min(left, window.innerWidth - panelW - edgePad));

  panelPosition.value = {
    top,
    left,
    width: rect.width,
  };
}

function toggle() {
  if (props.disabled) return;
  open.value = !open.value;
  if (open.value) {
    searchQuery.value = "";
    // Подсветим текущий выбранный (или первый) элемент.
    const idx = props.options.findIndex((o) => o.value === props.modelValue);
    highlightIdx.value = idx >= 0 ? idx : 0;
    nextTick(() => {
      reposition();
      // Auto-focus в search field (если есть) — UX как в macOS dropdown.
      if (isSearchable.value) searchInputRef.value?.focus();
    });
  }
}

function pick(opt: Option<T>) {
  if (opt.disabled) return;
  emit("update:modelValue", opt.value);
  open.value = false;
}

function onKey(e: KeyboardEvent) {
  if (!open.value) return;
  if (e.key === "Escape") {
    e.preventDefault();
    open.value = false;
    return;
  }
  const opts = filteredOptions.value;
  if (e.key === "ArrowDown") {
    e.preventDefault();
    for (let i = highlightIdx.value + 1; i < opts.length; i++) {
      if (!opts[i].disabled) {
        highlightIdx.value = i;
        return;
      }
    }
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    for (let i = highlightIdx.value - 1; i >= 0; i--) {
      if (!opts[i].disabled) {
        highlightIdx.value = i;
        return;
      }
    }
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    const opt = opts[highlightIdx.value];
    if (opt) pick(opt);
    return;
  }
}

// При фильтрации сбрасываем highlight на первую видимую опцию.
watch(searchQuery, () => {
  if (filteredOptions.value.length === 0) {
    highlightIdx.value = -1;
  } else {
    highlightIdx.value = 0;
  }
});

function onDocPointerDown(e: PointerEvent) {
  if (!open.value) return;
  const t = e.target;
  if (t instanceof Node) {
    if (triggerRef.value?.contains(t)) return;
    if (panelRef.value?.contains(t)) return;
  }
  open.value = false;
}

function onWindowResize() {
  if (open.value) reposition();
}

watch(open, (isOpen) => {
  if (isOpen) {
    document.addEventListener("pointerdown", onDocPointerDown);
    document.addEventListener("keydown", onKey);
    window.addEventListener("resize", onWindowResize);
    window.addEventListener("scroll", onWindowResize, true);
  } else {
    document.removeEventListener("pointerdown", onDocPointerDown);
    document.removeEventListener("keydown", onKey);
    window.removeEventListener("resize", onWindowResize);
    window.removeEventListener("scroll", onWindowResize, true);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointerDown);
  document.removeEventListener("keydown", onKey);
  window.removeEventListener("resize", onWindowResize);
  window.removeEventListener("scroll", onWindowResize, true);
});
</script>

<template>
  <div class="kosmos-dd">
    <button
      ref="triggerRef"
      type="button"
      class="kosmos-dd__trigger"
      :class="{
        'kosmos-dd__trigger--open': open,
        'kosmos-dd__trigger--placeholder': !selectedOption,
        'kosmos-dd__trigger--disabled': disabled,
      }"
      :disabled="disabled"
      :aria-haspopup="'listbox'"
      :aria-expanded="open"
      @click="toggle"
    >
      <!-- Слот для leading-иконки в trigger'е (показывает иконку текущего
           выбранного варианта). Если не передан — без иконки. -->
      <slot name="trigger-leading" :option="selectedOption" />
      <span class="kosmos-dd__label">{{ displayLabel }}</span>
      <ChevronDown
        class="kosmos-dd__chevron"
        :class="{ 'kosmos-dd__chevron--open': open }"
        :size="14"
        :stroke-width="2"
      />
    </button>

    <Teleport to="body">
      <transition name="kosmos-dd">
        <div
          v-if="open"
          ref="panelRef"
          class="kosmos-dd__panel"
          role="listbox"
          :style="{
            top: panelPosition.top + 'px',
            left: panelPosition.left + 'px',
            width: matchTriggerWidth ? panelPosition.width + 'px' : undefined,
          }"
        >
          <div v-if="isSearchable" class="kosmos-dd__search">
            <input
              ref="searchInputRef"
              v-model="searchQuery"
              type="text"
              class="kosmos-dd__search-input"
              :placeholder="searchPlaceholder"
              spellcheck="false"
              autocomplete="off"
            />
          </div>
          <div class="kosmos-dd__options kosmos-scroll">
            <button
              v-for="(opt, i) in filteredOptions"
              :key="String(opt.value)"
              type="button"
              class="kosmos-dd__option"
              :class="{
                'kosmos-dd__option--selected': opt.value === modelValue,
                'kosmos-dd__option--highlighted': i === highlightIdx,
                'kosmos-dd__option--disabled': opt.disabled,
              }"
              role="option"
              :aria-selected="opt.value === modelValue"
              :disabled="opt.disabled"
              @mouseenter="!opt.disabled && (highlightIdx = i)"
              @click="pick(opt)"
            >
              <!-- Слот для кастомной leading-иконки (например ProviderIcon).
                   Если не передан — просто label. -->
              <slot name="option-leading" :option="opt" />
              <span class="kosmos-dd__option-label">{{ opt.label }}</span>
              <Check
                v-if="opt.value === modelValue"
                class="kosmos-dd__check"
                :size="14"
                :stroke-width="2.4"
              />
            </button>
            <div v-if="filteredOptions.length === 0" class="kosmos-dd__empty">
              Ничего не найдено
            </div>
          </div>
        </div>
      </transition>
    </Teleport>
  </div>
</template>

<style scoped>
.kosmos-dd {
  position: relative;
  display: inline-flex;
  width: fit-content;
  max-width: 100%;
}

.kosmos-dd__trigger {
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  width: fit-content;
  max-width: 100%;
  height: 34px;
  padding: 0 0.625rem;
  background: transparent;
  border: 2px solid transparent;
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: inherit;
  font-size: 0.875rem;
  cursor: default;
  transition:
    border-color 140ms cubic-bezier(0.2, 0, 0, 1),
    background-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-dd__trigger:hover:not(.kosmos-dd__trigger--disabled) {
  border-color: #ffffff;
}

.kosmos-dd__trigger--open {
  border-color: color-mix(in srgb, var(--accent) 65%, transparent);
}

.kosmos-dd__trigger--placeholder {
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
}

.kosmos-dd__trigger--disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.kosmos-dd__label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  text-align: left;
}

.kosmos-dd__chevron {
  flex-shrink: 0;
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  transition: transform 180ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-dd__chevron--open {
  transform: rotate(-180deg);
}

.kosmos-dd__panel {
  position: fixed;
  z-index: 9500;
  display: flex;
  flex-direction: column;
  padding: 0;
  min-width: 200px;
  max-height: min(200px, calc(100vh - 32px));
  background: var(--popover, color-mix(in srgb, var(--background) 92%, black));
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
  border-radius: calc(var(--radius) * 0.85);
  corner-shape: var(--corner-shape);
  box-shadow:
    0 16px 40px rgb(0 0 0 / 36%),
    0 6px 16px rgb(0 0 0 / 18%);
  overflow: hidden;
}

.kosmos-dd__search {
  padding: 8px 8px 4px;
}

.kosmos-dd__search-input {
  width: 100%;
  height: 30px;
  padding: 0 10px;
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  border: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  border-radius: calc(var(--radius) * 0.55);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: inherit;
  font-size: 0.8125rem;
  outline: none;
  transition: border-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-dd__search-input:focus {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
}

.kosmos-dd__search-input::placeholder {
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
}

.kosmos-dd__options {
  display: flex;
  flex-direction: column;
  padding: 4px;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
}

.kosmos-dd__option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 8px 10px;
  background: transparent;
  border: none;
  border-radius: calc(var(--radius) * 0.55);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: inherit;
  font-size: 0.875rem;
  font-weight: 500;
  text-align: left;
  cursor: default;
  transition: background-color 100ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-dd__option--highlighted {
  background: color-mix(in srgb, var(--foreground) 14%, transparent);
}

.kosmos-dd__option--selected {
  background: color-mix(in srgb, var(--foreground) 14%, transparent);
  color: var(--foreground);
}

/* Раздельная подсветка: пока юзер не двигает курсор / клавиатуру — fill
 * стоит на выбранном (--selected). Как только highlightIdx сменяется (hover
 * или arrow keys на ДРУГОЙ option) — selected теряет фон, fill переезжает
 * на highlight. Создаёт иллюзию "движущегося индикатора". */
.kosmos-dd__options:has(.kosmos-dd__option--highlighted)
  .kosmos-dd__option--selected:not(.kosmos-dd__option--highlighted) {
  background: transparent;
}

.kosmos-dd__option--disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.kosmos-dd__empty {
  padding: 12px 10px;
  text-align: center;
  font-size: 0.8125rem;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
}

.kosmos-dd__option-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.kosmos-dd__check {
  flex-shrink: 0;
  color: var(--accent);
}

.kosmos-dd-enter-active,
.kosmos-dd-leave-active {
  transition:
    opacity 140ms cubic-bezier(0.2, 0, 0, 1),
    transform 140ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-dd-enter-from,
.kosmos-dd-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
