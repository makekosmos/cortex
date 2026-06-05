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
  /** Максимальная высота popup в px. По умолчанию 200. */
  maxHeightPx?: number;
}

const props = withDefaults(defineProps<Props<T>>(), {
  placeholder: "Выбрать…",
  matchTriggerWidth: true,
  disabled: false,
  searchable: "auto",
  searchPlaceholder: "Поиск…",
  maxHeightPx: 200,
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
  <div class="relative inline-flex w-fit max-w-full">
    <button
      ref="triggerRef"
      type="button"
      class="inline-flex h-10 w-fit max-w-full items-center justify-between gap-2 rounded-[var(--radius-input)] border-2 border-transparent bg-transparent px-2 font-[inherit] text-[length:var(--kosmos-text-control-size)] text-[var(--foreground)] transition-[background-color,border-color] duration-140 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] hover:not-disabled:border-white"
      :class="{
        'border-[color-mix(in_srgb,var(--accent)_65%,transparent)]': open,
        'text-[color-mix(in_srgb,var(--foreground)_50%,transparent)]': !selectedOption,
        'cursor-not-allowed opacity-[0.55] hover:border-transparent': disabled,
      }"
      :disabled="disabled"
      :aria-haspopup="'listbox'"
      :aria-expanded="open"
      @click="toggle"
    >
      <!-- Слот для leading-иконки в trigger'е (показывает иконку текущего
           выбранного варианта). Если не передан — без иконки. -->
      <slot name="trigger-leading" :option="selectedOption" />
      <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-left">
        {{ displayLabel }}
      </span>
      <ChevronDown
        class="shrink-0 text-[color-mix(in_srgb,var(--foreground)_65%,transparent)] transition-transform duration-[180ms] ease-[cubic-bezier(0.2,0,0,1)]"
        :class="{ '-rotate-180': open }"
        :size="14"
        :stroke-width="2"
      />
    </button>

    <Teleport to="body">
      <transition name="kosmos-dd">
        <div
          v-if="open"
          ref="panelRef"
          class="fixed z-[9500] flex min-w-[200px] flex-col overflow-hidden rounded-[var(--radius-button)] border border-[color-mix(in_srgb,var(--border)_80%,transparent)] bg-[var(--popover,color-mix(in_srgb,var(--background)_92%,black))] p-0 shadow-[0_16px_40px_color-mix(in_srgb,var(--background)_36%,transparent),0_8px_16px_color-mix(in_srgb,var(--background)_18%,transparent)] backdrop-blur-[20px] backdrop-saturate-[180%] [corner-shape:var(--corner-shape)]"
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
              class="h-8 w-full rounded-[var(--radius-input)] border border-[color-mix(in_srgb,var(--border)_60%,transparent)] bg-[color-mix(in_srgb,var(--foreground)_6%,transparent)] px-2 font-[inherit] text-[length:var(--kosmos-text-body-size)] text-[var(--foreground)] outline-none transition-colors duration-140 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] placeholder:text-[color-mix(in_srgb,var(--foreground)_45%,transparent)] focus:border-[color-mix(in_srgb,var(--accent)_55%,var(--border))]"
              :placeholder="searchPlaceholder"
              spellcheck="false"
              autocomplete="off"
            />
          </div>
          <div
            class="kosmos-dd__options kosmos-scroll flex min-h-0 flex-1 flex-col overflow-y-auto p-1"
          >
            <button
              v-for="(opt, i) in filteredOptions"
              :key="String(opt.value)"
              type="button"
              class="kosmos-dd__option flex w-full items-center gap-2 rounded-[var(--radius-input)] border-0 bg-transparent px-2 py-2 text-left font-[inherit] text-[length:var(--kosmos-text-control-size)] font-medium text-[var(--foreground)] transition-colors duration-100 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)]"
              :class="{
                'kosmos-dd__option--selected': opt.value === modelValue,
                'kosmos-dd__option--highlighted': i === highlightIdx,
                'bg-[color-mix(in_srgb,var(--foreground)_14%,transparent)]':
                  opt.value === modelValue || i === highlightIdx,
                'cursor-not-allowed opacity-45': opt.disabled,
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
              <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">
                {{ opt.label }}
              </span>
              <Check
                v-if="opt.value === modelValue"
                class="shrink-0 text-[var(--accent)]"
                :size="14"
                :stroke-width="2.4"
              />
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
/* Раздельная подсветка: пока юзер не двигает курсор / клавиатуру — fill
 * стоит на выбранном (--selected). Как только highlightIdx сменяется (hover
 * или arrow keys на ДРУГОЙ option) — selected теряет фон, fill переезжает
 * на highlight. Создаёт иллюзию "движущегося индикатора". */
.kosmos-dd__options:has(.kosmos-dd__option--highlighted)
  .kosmos-dd__option--selected:not(.kosmos-dd__option--highlighted) {
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
