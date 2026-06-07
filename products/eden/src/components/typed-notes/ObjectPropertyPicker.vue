<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef, watch } from "vue";

interface PickerOption {
  value: string;
  label: string;
}

const props = withDefaults(
  defineProps<{
    modelValue: string | string[] | null | undefined;
    options: PickerOption[];
    placeholder: string;
    variant?: "featured-inline" | "featured-column" | "secondary";
    multiple?: boolean;
    disabled?: boolean;
    emptyLabel?: string;
    emptyOptionsLabel?: string;
  }>(),
  {
    variant: "secondary",
    multiple: false,
    disabled: false,
    emptyLabel: "Без значения",
    emptyOptionsLabel: "Нет вариантов",
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: string | string[]];
}>();

const isOpen = shallowRef(false);
const rootRef = useTemplateRef<HTMLDivElement>("root");
const triggerRef = useTemplateRef<HTMLButtonElement>("trigger");
const panelRef = useTemplateRef<HTMLDivElement>("panel");
const panelTop = shallowRef(0);
const panelLeft = shallowRef(0);
const panelWidth = shallowRef(220);
const panelMaxHeight = shallowRef(260);

const selectedValues = computed(() => {
  if (Array.isArray(props.modelValue)) {
    return props.modelValue.filter(
      (value): value is string => typeof value === "string" && value.trim().length > 0,
    );
  }

  if (typeof props.modelValue === "string" && props.modelValue.trim().length > 0) {
    return [props.modelValue];
  }

  return [];
});

const optionLabels = computed(
  () =>
    new Map(props.options.map((option) => [option.value, option.label] satisfies [string, string])),
);
const selectedOptions = computed(() =>
  selectedValues.value.map((value) => ({
    value,
    label: optionLabels.value.get(value) ?? value,
  })),
);
const selectedLabels = computed(() => selectedOptions.value.map((option) => option.label));
const hasSelection = computed(() => selectedLabels.value.length > 0);
const summaryText = computed(() => selectedLabels.value.join(", "));
const panelStyle = computed(() => ({
  top: `${panelTop.value}px`,
  left: `${panelLeft.value}px`,
  width: `${panelWidth.value}px`,
  maxHeight: `${panelMaxHeight.value}px`,
}));

function updatePanelPosition() {
  const rect = triggerRef.value?.getBoundingClientRect();
  if (!rect) {
    return;
  }

  const viewportWidth = window.innerWidth;
  const viewportHeight = window.innerHeight;
  const horizontalPadding = 8;
  const verticalGap = 6;
  const desiredWidth = props.variant === "featured-inline" ? Math.max(rect.width, 220) : rect.width;
  const width = Math.min(desiredWidth, Math.max(220, viewportWidth - horizontalPadding * 2));
  const left = Math.min(
    Math.max(horizontalPadding, rect.left),
    Math.max(horizontalPadding, viewportWidth - width - horizontalPadding),
  );
  const top = Math.round(rect.bottom + verticalGap);
  const maxHeight = Math.max(160, viewportHeight - top - 12);

  panelTop.value = top;
  panelLeft.value = Math.round(left);
  panelWidth.value = Math.round(width);
  panelMaxHeight.value = Math.round(maxHeight);
}

function closePicker() {
  isOpen.value = false;
}

function togglePicker() {
  if (props.disabled) {
    return;
  }

  isOpen.value = !isOpen.value;
}

function selectValue(value: string) {
  if (props.disabled) {
    return;
  }

  if (props.multiple) {
    const next = new Set(selectedValues.value);
    if (next.has(value)) {
      next.delete(value);
    } else {
      next.add(value);
    }

    emit("update:modelValue", [...next]);
    return;
  }

  emit("update:modelValue", value);
  closePicker();
}

function clearSingleValue() {
  emit("update:modelValue", "");
  closePicker();
}

function isSelected(value: string) {
  return selectedValues.value.includes(value);
}

watch(isOpen, async (open, _previous, onCleanup) => {
  if (!open) {
    return;
  }

  await nextTick();
  updatePanelPosition();

  const handlePointerDown = (event: MouseEvent) => {
    if (
      rootRef.value?.contains(event.target as Node) ||
      panelRef.value?.contains(event.target as Node)
    ) {
      return;
    }

    closePicker();
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      closePicker();
    }
  };

  const handleViewportChange = () => {
    updatePanelPosition();
  };

  window.addEventListener("pointerdown", handlePointerDown);
  window.addEventListener("keydown", handleKeyDown);
  window.addEventListener("resize", handleViewportChange);
  window.addEventListener("scroll", handleViewportChange, true);

  onCleanup(() => {
    window.removeEventListener("pointerdown", handlePointerDown);
    window.removeEventListener("keydown", handleKeyDown);
    window.removeEventListener("resize", handleViewportChange);
    window.removeEventListener("scroll", handleViewportChange, true);
  });
});
</script>

<template>
  <div
    ref="root"
    class="object-property-picker"
    :class="[
      `object-property-picker--${variant}`,
      multiple && 'object-property-picker--multiple',
      isOpen && 'is-open',
      disabled && 'is-disabled',
    ]"
  >
    <button
      ref="trigger"
      type="button"
      class="object-property-picker__trigger"
      :disabled="disabled"
      :aria-expanded="isOpen ? 'true' : 'false'"
      @click="togglePicker"
    >
      <span v-if="hasSelection && !multiple" class="object-property-picker__summary">
        {{ summaryText }}
      </span>

      <span v-else-if="hasSelection" class="object-property-picker__tokens">
        <span
          v-for="option in selectedOptions"
          :key="option.value"
          class="object-property-picker__token"
        >
          {{ option.label }}
        </span>
      </span>

      <span v-else class="object-property-picker__placeholder">
        {{ placeholder }}
      </span>

      <span class="object-property-picker__chevron" aria-hidden="true">›</span>
    </button>

    <Teleport to="body">
      <div
        v-if="isOpen"
        ref="panel"
        class="object-property-picker__panel"
        :class="`object-property-picker__panel--${variant}`"
        :style="panelStyle"
      >
        <button
          v-if="!multiple"
          type="button"
          class="object-property-picker__option"
          :class="!hasSelection && 'is-active'"
          @click="clearSingleValue"
        >
          <span class="object-property-picker__check" :class="!hasSelection && 'is-active'"></span>
          <span class="object-property-picker__option-label">{{ emptyLabel }}</span>
        </button>

        <div v-if="options.length > 0" class="object-property-picker__options">
          <button
            v-for="option in options"
            :key="option.value"
            type="button"
            class="object-property-picker__option"
            :class="isSelected(option.value) && 'is-active'"
            @click="selectValue(option.value)"
          >
            <span
              class="object-property-picker__check"
              :class="isSelected(option.value) && 'is-active'"
            ></span>
            <span class="object-property-picker__option-label">{{ option.label }}</span>
          </button>
        </div>

        <div v-else class="object-property-picker__empty">
          {{ emptyOptionsLabel }}
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.object-property-picker {
  position: relative;
  width: 100%;
  min-width: 0;
}

.object-property-picker__trigger {
  width: 100%;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  min-height: 24px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--foreground);
  font: inherit;
  line-height: inherit;
  text-align: left;
}

.object-property-picker--multiple .object-property-picker__trigger {
  align-items: start;
}

.object-property-picker__trigger:disabled {
  cursor: default;
  opacity: 0.72;
}

.object-property-picker__summary,
.object-property-picker__placeholder {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.object-property-picker__placeholder {
  color: var(--muted-foreground);
}

.object-property-picker__tokens {
  min-width: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.object-property-picker__token {
  display: inline-flex;
  align-items: center;
  min-height: 24px;
  padding: 0 10px;
  border-radius: 999px;
  background: var(--secondary);
  color: var(--secondary-foreground);
}

.object-property-picker__chevron {
  flex-shrink: 0;
  color: var(--muted-foreground);
  font-size: 14px;
  line-height: 1;
  transform: rotate(90deg);
  transition:
    transform 0.16s ease,
    color 0.16s ease;
}

.object-property-picker.is-open .object-property-picker__chevron {
  transform: rotate(270deg);
  color: var(--foreground);
}

.object-property-picker--multiple .object-property-picker__chevron {
  margin-top: 4px;
}

.object-property-picker__panel {
  position: fixed;
  z-index: 4000;
  display: grid;
  gap: 4px;
  padding: 8px;
  overflow: hidden;
  border-radius: 14px;
  border: 1px solid var(--border-glass, var(--border));
  background: var(--bg-elevated, var(--popover));
  box-shadow: var(--shadow-lg, 0 20px 40px rgb(0 0 0 / 0.24));
  backdrop-filter: var(--blur-lg, blur(14px));
  -webkit-backdrop-filter: var(--blur-lg, blur(14px));
}

.object-property-picker__options {
  display: grid;
  gap: 4px;
  max-height: inherit;
  overflow-y: auto;
}

.object-property-picker__option {
  width: 100%;
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr);
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: none;
  border-radius: 10px;
  background: transparent;
  color: var(--text-secondary, var(--muted-foreground));
  font: inherit;
  text-align: left;
  transition:
    background-color 0.16s ease,
    color 0.16s ease;
}

.object-property-picker__option:hover,
.object-property-picker__option.is-active {
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-primary, var(--foreground));
}

.object-property-picker__check {
  width: 16px;
  height: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid color-mix(in srgb, var(--border) 86%, transparent);
  border-radius: 999px;
  background: transparent;
}

.object-property-picker__check.is-active {
  border-color: color-mix(in srgb, var(--ring) 72%, var(--border));
  background: color-mix(in srgb, var(--ring) 72%, transparent);
}

.object-property-picker__check.is-active::after {
  content: "";
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: white;
}

.object-property-picker__option-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.object-property-picker__empty {
  padding: 10px;
  color: var(--muted-foreground);
  font: inherit;
}
</style>
