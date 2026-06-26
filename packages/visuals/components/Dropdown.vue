<script setup lang="ts" generic="T extends string | number">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { ChevronDown } from "@lucide/vue";

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
  /** Monochrome mask icon for the option. */
  iconSrc?: string;
  /** Color for iconSrc/dot. */
  color?: string;
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
  /** Максимальная высота popup в px. По умолчанию 200. */
  maxHeightPx?: number;
  /** Выравнивание popup относительно trigger'а. По умолчанию — end. */
  panelAlign?: "start" | "end";
  /** Показывать chevron в trigger'е. */
  showChevron?: boolean;
  /** Создать виртуальную опцию из search query, если её нет в списке. */
  createOption?: (query: string) => Option<V> | null;
}

const props = withDefaults(defineProps<Props<T>>(), {
  placeholder: "Выбрать…",
  matchTriggerWidth: true,
  disabled: false,
  searchable: "auto",
  searchPlaceholder: "Поиск…",
  maxHeightPx: 200,
  panelAlign: "end",
  showChevron: true,
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

const filteredOptions = computed<Option<T>[]>(() => {
  const rawQuery = searchQuery.value.trim();
  const q = rawQuery.toLowerCase();
  if (!q) return props.options;

  const matches = props.options.filter(
    (o) =>
      o.label.toLowerCase().includes(q) ||
      (o.description && o.description.toLowerCase().includes(q)),
  );
  const exactMatch = matches.some(
    (o) => o.label.toLowerCase() === q || String(o.value) === rawQuery,
  );
  const created = exactMatch ? null : props.createOption?.(rawQuery);

  return created ? [created, ...matches] : matches;
});

const selectedOption = computed<Option<T> | null>(() => {
  if (props.modelValue === null || props.modelValue === undefined) return null;
  return props.options.find((o) => o.value === props.modelValue) ?? null;
});

const displayLabel = computed(() =>
  selectedOption.value ? selectedOption.value.label : props.placeholder,
);

const panelMaxHeight = computed(() => Math.max(80, Math.min(420, props.maxHeightPx)));

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
  // расходится влево). Для inline/property fields можно выбрать start,
  // чтобы popup начинался от левого края trigger'а.
  let left = props.panelAlign === "start" ? rect.left : rect.right - panelW;
  if (props.panelAlign === "end" && left < edgePad) {
    left = rect.left;
  }
  if (props.panelAlign === "start" && left + panelW > window.innerWidth - edgePad) {
    left = rect.right - panelW;
  }
  // Final clamp в обе стороны viewport.
  left = Math.max(edgePad, Math.min(left, window.innerWidth - panelW - edgePad));

  panelPosition.value = {
    top,
    left,
    width: rect.width,
  };
}

function scrollSelectedOptionIntoView() {
  panelRef.value
    ?.querySelector<HTMLElement>(".kosmos-dd__option--selected")
    ?.scrollIntoView({ block: "nearest" });
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
      scrollSelectedOptionIntoView();
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
  <div class="relative inline-flex w-fit max-w-full">
    <button
      ref="triggerRef"
      type="button"
      class="inline-flex h-7 w-fit max-w-full items-center justify-between gap-1.5 rounded-[var(--radius-input)] border border-transparent bg-transparent px-1.5 font-[inherit] font-medium leading-[1.2] text-[var(--foreground)] transition-[background-color,border-color] duration-140 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] hover:not-disabled:border-[color-mix(in_srgb,var(--foreground)_18%,transparent)]"
      :class="{
        'border-[color-mix(in_srgb,var(--accent)_65%,transparent)]': open,
        'text-[color-mix(in_srgb,var(--foreground)_50%,transparent)]': !selectedOption,
        'cursor-not-allowed opacity-[0.55] hover:border-transparent': disabled,
      }"
      :disabled="disabled"
      :aria-haspopup="'listbox'"
      :aria-expanded="open"
      :style="{ fontSize: 'var(--kosmos-settings-font-size-base, 12px)' }"
      @click="toggle"
    >
      <!-- Слот для leading-иконки в trigger'е (показывает иконку текущего
           выбранного варианта). Если не передан — без иконки. -->
      <slot name="trigger-leading" :option="selectedOption" />
      <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-left">
        {{ displayLabel }}
      </span>
      <ChevronDown
        v-if="showChevron"
        class="shrink-0 text-[color-mix(in_srgb,var(--foreground)_65%,transparent)] transition-transform duration-[180ms] ease-[cubic-bezier(0.2,0,0,1)]"
        :class="{ '-rotate-180': open }"
        :size="12"
        :stroke-width="2"
      />
    </button>

    <Teleport to="body">
      <transition name="kosmos-dd">
        <div
          v-if="open"
          ref="panelRef"
          class="kosmos-dd__panel fixed z-[9500] flex min-w-[200px] flex-col overflow-hidden rounded-xl border border-[var(--border-color-strong,var(--border))] bg-[var(--settings-search-surface,var(--popover,var(--background)))] p-0 text-[var(--foreground)] [corner-shape:var(--corner-shape)]"
          role="listbox"
          :style="{
            top: panelPosition.top + 'px',
            left: panelPosition.left + 'px',
            width: matchTriggerWidth ? panelPosition.width + 'px' : undefined,
            maxHeight: `min(${panelMaxHeight}px, calc(100vh - 32px))`,
          }"
        >
          <div v-if="isSearchable" class="px-2 pt-2 pb-1">
            <input
              ref="searchInputRef"
              v-model="searchQuery"
              type="text"
              class="h-8 w-full rounded-[var(--radius-input)] border border-[var(--border-color-strong,var(--border))] bg-[color-mix(in_srgb,var(--foreground)_6%,transparent)] px-2 font-[inherit] text-[var(--foreground)] outline-none transition-colors duration-140 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] placeholder:text-[color-mix(in_srgb,var(--foreground)_45%,transparent)] focus:border-[color-mix(in_srgb,var(--foreground)_18%,transparent)]"
              :placeholder="searchPlaceholder"
              spellcheck="false"
              autocomplete="off"
              :style="{ fontSize: 'var(--kosmos-settings-font-size-base, 12px)' }"
            />
          </div>
          <div
            class="kosmos-dd__options kosmos-scroll flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto py-2"
          >
            <button
              v-for="(opt, i) in filteredOptions"
              :key="String(opt.value)"
              type="button"
              class="kosmos-dd__option flex min-h-8 w-full items-center gap-2 rounded-[var(--radius-input)] border-0 bg-transparent text-left font-[inherit] font-medium text-[var(--foreground)] transition-colors duration-100 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)]"
              :class="{
                'kosmos-dd__option--selected': opt.value === modelValue,
                'kosmos-dd__option--highlighted': i === highlightIdx,
                'cursor-not-allowed opacity-45': opt.disabled,
              }"
              role="option"
              :aria-selected="opt.value === modelValue"
              :disabled="opt.disabled"
              :style="{ fontSize: 'var(--kosmos-settings-font-size-base, 12px)' }"
              @mouseenter="!opt.disabled && (highlightIdx = i)"
              @click="pick(opt)"
            >
              <!-- Слот для кастомной leading-иконки (например ProviderIcon).
                   Если не передан — опция может сама отрисовать iconSrc/color. -->
              <slot name="option-leading" :option="opt">
                <span
                  v-if="opt.iconSrc || opt.color"
                  class="kosmos-dd__option-swatch"
                  :style="{ '--kosmos-dd-option-color': opt.color ?? 'var(--text-secondary)' }"
                  aria-hidden="true"
                >
                  <span
                    v-if="opt.iconSrc"
                    class="kosmos-dd__option-icon"
                    :style="{ '--kosmos-dd-option-icon-src': `url(${opt.iconSrc})` }"
                  ></span>
                  <span v-else class="kosmos-dd__option-dot"></span>
                </span>
              </slot>
              <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">
                {{ opt.label }}
              </span>
            </button>
            <div
              v-if="filteredOptions.length === 0"
              class="px-2 py-4 text-center text-[0.8125rem] text-[color-mix(in_srgb,var(--foreground)_50%,transparent)]"
            >
              Ничего не найдено
            </div>
          </div>
        </div>
      </transition>
    </Teleport>
  </div>
</template>

<style scoped>
.kosmos-dd__options {
  scrollbar-gutter: stable both-edges;
}

.kosmos-dd__option {
  padding: 6px 10px;
}

.kosmos-dd__option-swatch {
  width: 18px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: var(--kosmos-dd-option-color);
}

.kosmos-dd__option-icon {
  width: 14px;
  height: 14px;
  display: block;
  background-color: var(--kosmos-dd-option-color);
  -webkit-mask-image: var(--kosmos-dd-option-icon-src);
  mask-image: var(--kosmos-dd-option-icon-src);
  -webkit-mask-repeat: no-repeat;
  mask-repeat: no-repeat;
  -webkit-mask-position: center;
  mask-position: center;
  -webkit-mask-size: contain;
  mask-size: contain;
}

.kosmos-dd__option-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: var(--kosmos-dd-option-color);
}

.kosmos-dd__option:hover,
.kosmos-dd__option:focus-visible,
.kosmos-dd__option--highlighted,
.kosmos-dd__option--selected {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.kosmos-dd__options:has(.kosmos-dd__option--highlighted:not(.kosmos-dd__option--selected))
  .kosmos-dd__option--selected:not(.kosmos-dd__option--highlighted) {
  background: transparent;
}

.kosmos-dd__option:disabled,
.kosmos-dd__option:disabled:hover,
.kosmos-dd__option:disabled:focus-visible {
  background: transparent;
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
  transform: translateY(-8px);
}
</style>
