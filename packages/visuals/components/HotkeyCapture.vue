<script setup lang="ts">
// HotkeyCapture — UI primitive для назначения accelerator'а.
//
// Два режима capture:
//   1. Локальный (default) — слушает DOM keydown сам. Работает для
//      несистемных shortcut'ов (Ctrl+Shift+;, Alt+D, etc.).
//   2. Внешний — компонент только показывает UI «жду нажатие», а событие
//      приходит через prop'ы. Используется когда нужно ловить системные
//      shortcut'ы (Win+H, Win+Space) через нижестоящий keyboard hook
//      ОС-уровня — иначе системный shortcut срабатывает раньше WebContents
//      keyboard handler'а.
//
// Внешний режим включается передачей prop `externalCapture: true`. В этом
// случае parent сам стартует capture в системе (например через
// `dictation.begin_hotkey_capture` backend op'у) и передаёт результат
// обратно через `pendingAccelerator` / `pendingCancel` events.

import { computed, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    /** Текст в плейсхолдере когда нет hotkey. */
    placeholder?: string;
    /** Текст пока ждём нажатие. */
    capturePrompt?: string;
    disabled?: boolean;
    /** Если true — компонент НЕ слушает DOM keydown сам. Capture лифт
     * наружу: parent стартует системный hook (например через ARK
     * `dictation.begin_hotkey_capture`) когда срабатывает `@capture-start`,
     * прокидывает результат назад установкой `pendingAccelerator`. */
    externalCapture?: boolean;
    /** Внешне-полученный accelerator (от parent'а в externalCapture mode).
     * При изменении на не-пустую строку — emit'им `update:modelValue` и
     * выходим из capture state. */
    pendingAccelerator?: string | null;
    /** Cancel signal от parent'а (например юзер нажал Escape — backend
     * прислал `dictation_capture_cancelled`). Toggle для тригера. */
    pendingCancel?: number;
  }>(),
  {
    placeholder: "Не задано",
    capturePrompt: "Нажмите сочетание…",
    disabled: false,
    externalCapture: false,
    pendingAccelerator: null,
    pendingCancel: 0,
  },
);

const emit = defineEmits<{
  "update:modelValue": [v: string];
  /** Срабатывает при Escape — parent может отреагировать (например clear). */
  cancel: [];
  /** В externalCapture mode — parent должен стартовать системный hook. */
  "capture-start": [];
  /** В externalCapture mode — parent должен остановить системный hook
   *  (например при blur'е окна или unmount). */
  "capture-end": [];
}>();

const capturing = ref(false);
const buttonRef = ref<HTMLButtonElement | null>(null);

// На macOS модификаторы показываем нативными символами (⌘ ⌥ ⌃ ⇧), на остальных
// платформах — текстом. Super/Meta = Command на macOS, Win на Windows.
const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform);

const keyParts = computed(() => {
  if (!props.modelValue) return [];
  const labels: Record<string, string> = isMac
    ? {
        Ctrl: "⌃",
        Control: "⌃",
        Alt: "⌥",
        Shift: "⇧",
        Super: "⌘",
        Meta: "⌘",
        Space: "Space",
      }
    : {
        Ctrl: "Ctrl",
        Control: "Ctrl",
        Alt: "Alt",
        Shift: "Shift",
        Super: "Win",
        Meta: "Win",
        Space: "Space",
      };
  return props.modelValue
    .split("+")
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => labels[part] ?? part);
});

function start() {
  if (props.disabled) return;
  capturing.value = true;
  // macOS: клик по <button> по умолчанию НЕ ставит focus (WebKit/Chromium
  // следует системной настройке Full Keyboard Access). Без focus button не
  // получает keydown → onKey не срабатывает → капчур висит и не ловит клавиши.
  // Программный .focus() работает на всех платформах.
  buttonRef.value?.focus();
  if (props.externalCapture) {
    emit("capture-start");
  }
}

function stop() {
  if (capturing.value && props.externalCapture) {
    emit("capture-end");
  }
  capturing.value = false;
}

// External capture: реактивно ловим accelerator или cancel от parent'а.
watch(
  () => props.pendingAccelerator,
  (v) => {
    if (!props.externalCapture) return;
    if (v && capturing.value) {
      emit("update:modelValue", v);
      capturing.value = false;
    }
  },
);
watch(
  () => props.pendingCancel,
  () => {
    if (!props.externalCapture) return;
    if (capturing.value) {
      capturing.value = false;
      emit("cancel");
    }
  },
);

function keyEventToAccelerator(e: KeyboardEvent): string | null {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  const key = e.key;
  if (key === "Control" || key === "Alt" || key === "Shift" || key === "Meta") {
    return null;
  }
  let main: string;
  if (key === " ") main = "Space";
  else if (key === "Escape") return "ESC_CANCEL";
  else if (key.length === 1) main = key.toUpperCase();
  else main = key;
  parts.push(main);
  return parts.join("+");
}

function onKey(e: KeyboardEvent) {
  if (!capturing.value) return;
  // В externalCapture parent сам ловит события через системный hook —
  // DOM keydown не trustworthy (системные shortcut'ы перехватываются до
  // того как WebContents получит event).
  if (props.externalCapture) return;
  e.preventDefault();
  e.stopPropagation();
  const acc = keyEventToAccelerator(e);
  if (!acc) return;
  if (acc === "ESC_CANCEL") {
    capturing.value = false;
    emit("cancel");
    return;
  }
  emit("update:modelValue", acc);
  capturing.value = false;
}
</script>

<template>
  <button
    ref="buttonRef"
    type="button"
    :class="[
      'inline-flex min-h-8 min-w-[120px] cursor-default items-center justify-center rounded-md border border-[color-mix(in_srgb,var(--foreground)_14%,transparent)] bg-[color-mix(in_srgb,var(--foreground)_6%,transparent)] px-2 font-[inherit] text-[length:var(--kosmos-text-caption-size)] text-[var(--foreground)] transition-[border-color,background-color] duration-140 ease-[cubic-bezier(0.2,0,0,1)] disabled:cursor-not-allowed disabled:opacity-55',
      capturing
        ? 'border-[color-mix(in_srgb,var(--accent)_65%,transparent)] bg-[color-mix(in_srgb,var(--accent)_8%,transparent)]'
        : '',
      !capturing
        ? 'hover:not-disabled:border-[color-mix(in_srgb,var(--foreground)_35%,transparent)] hover:not-disabled:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)]'
        : '',
    ]"
    :disabled="disabled"
    @click="start"
    @keydown="onKey"
    @blur="stop"
  >
    <span v-if="capturing" class="px-1 font-medium text-[var(--accent)]">{{ capturePrompt }}</span>
    <span v-else-if="keyParts.length > 0" class="flex items-center gap-1">
      <kbd
        v-for="part in keyParts"
        :key="part"
        class="inline-flex h-5 min-w-5 items-center justify-center rounded border border-[color-mix(in_srgb,var(--foreground)_14%,transparent)] bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] px-1 font-[var(--font-sans)] text-[length:var(--kosmos-text-caption-size)] leading-none font-medium text-[var(--foreground)] shadow-[inset_0_-1px_0_color-mix(in_srgb,var(--background)_22%,transparent)]"
      >
        {{ part }}
      </kbd>
    </span>
    <span v-else class="px-1 text-[color-mix(in_srgb,var(--foreground)_45%,transparent)]">
      {{ placeholder }}
    </span>
  </button>
</template>
