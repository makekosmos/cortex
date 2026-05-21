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
}

const props = withDefaults(defineProps<Props<T>>(), {
  placeholder: "Выбрать…",
  matchTriggerWidth: true,
  disabled: false,
});

const emit = defineEmits<{
  "update:modelValue": [v: T];
}>();

const open = ref(false);
const triggerRef = ref<HTMLElement | null>(null);
const panelRef = ref<HTMLElement | null>(null);
const panelPosition = ref<{ top: number; left: number; width: number }>({
  top: 0,
  left: 0,
  width: 0,
});
const highlightIdx = ref(0);

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
  const margin = 4;
  const spaceBelow = window.innerHeight - rect.bottom - margin;
  const spaceAbove = rect.top - margin;
  // По умолчанию ниже триггера. Если внизу не помещается — наверх.
  let top: number;
  if (panelH <= spaceBelow || spaceBelow >= spaceAbove) {
    top = rect.bottom + margin;
  } else {
    top = Math.max(rect.top - panelH - margin, 8);
  }
  panelPosition.value = {
    top,
    left: rect.left,
    width: rect.width,
  };
}

function toggle() {
  if (props.disabled) return;
  open.value = !open.value;
  if (open.value) {
    // Подсветим текущий выбранный (или первый) элемент.
    const idx = props.options.findIndex((o) => o.value === props.modelValue);
    highlightIdx.value = idx >= 0 ? idx : 0;
    nextTick(reposition);
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
  if (e.key === "ArrowDown") {
    e.preventDefault();
    for (let i = highlightIdx.value + 1; i < props.options.length; i++) {
      if (!props.options[i].disabled) {
        highlightIdx.value = i;
        return;
      }
    }
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    for (let i = highlightIdx.value - 1; i >= 0; i--) {
      if (!props.options[i].disabled) {
        highlightIdx.value = i;
        return;
      }
    }
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    const opt = props.options[highlightIdx.value];
    if (opt) pick(opt);
    return;
  }
}

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
          <button
            v-for="(opt, i) in options"
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
            <span class="kosmos-dd__option-main">
              <span class="kosmos-dd__option-label">{{ opt.label }}</span>
              <span v-if="opt.description" class="kosmos-dd__option-desc">{{
                opt.description
              }}</span>
            </span>
            <Check
              v-if="opt.value === modelValue"
              class="kosmos-dd__check"
              :size="14"
              :stroke-width="2.4"
            />
          </button>
        </div>
      </transition>
    </Teleport>
  </div>
</template>

<style scoped>
.kosmos-dd {
  position: relative;
  display: inline-flex;
  width: 100%;
}

.kosmos-dd__trigger {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  width: 100%;
  height: 34px;
  padding: 0 0.625rem;
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  border: 2px solid var(--border);
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: inherit;
  font-size: 0.875rem;
  cursor: pointer;
  transition:
    border-color 140ms cubic-bezier(0.2, 0, 0, 1),
    background-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-dd__trigger:hover:not(.kosmos-dd__trigger--disabled) {
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
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
  padding: 0.25rem;
  min-width: 180px;
  max-height: min(320px, calc(100vh - 32px));
  overflow-y: auto;
  background: var(--popover, var(--background));
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.85);
  corner-shape: var(--corner-shape);
  box-shadow:
    0 12px 32px rgb(0 0 0 / 28%),
    0 4px 12px rgb(0 0 0 / 14%);
}

.kosmos-dd__option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 0.5rem 0.625rem;
  background: transparent;
  border: none;
  border-radius: calc(var(--radius) * 0.55);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: inherit;
  font-size: 0.875rem;
  text-align: left;
  cursor: pointer;
  transition: background-color 100ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-dd__option--highlighted {
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
}

.kosmos-dd__option--selected {
  color: var(--accent);
  font-weight: 600;
}

.kosmos-dd__option--disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.kosmos-dd__option-main {
  display: flex;
  flex-direction: column;
  gap: 0.125rem;
  flex: 1;
  min-width: 0;
}

.kosmos-dd__option-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.kosmos-dd__option-desc {
  font-size: 0.75rem;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
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
