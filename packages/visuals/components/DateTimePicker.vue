<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { Calendar as CalendarIcon } from "@lucide/vue";
import Calendar from "./Calendar.vue";

interface Props {
  /** ISO timestamp (UTC). Null = пусто. */
  value: string | null;
  placeholder?: string;
  /** Заголовок над триггером (опционально). */
  label?: string;
  /**
   * Опорная дата для компактного отображения в триггере. Если совпадает
   * с `value` (тот же день/месяц/год) — соответствующие части скрываются.
   * Например, для пары пикеров «С/По» можно передать сюда `Date.now()`
   * (или start-значение в end-picker), и компактная подпись будет:
   * - тот же день → `HH:MM`
   * - другой день в том же месяце → `DD HH:MM`
   * - другой месяц в том же году → `DD.MM HH:MM`
   * - другой год → `DD.MM.YY HH:MM`
   * Если не передано — формат остаётся полным («13 мая 2026, 14:30»).
   */
  reference?: string | number | Date | null;
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: "Выбрать…",
  label: undefined,
  reference: null,
});

const emit = defineEmits<{
  "update:value": [iso: string | null];
}>();

const open = ref(false);
const triggerRef = ref<HTMLElement | null>(null);
const panelRef = ref<HTMLElement | null>(null);
const panelPosition = ref<{ top: number; left: number; placement: "below" | "above" }>({
  top: 0,
  left: 0,
  placement: "below",
});

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

interface DTState {
  year: number;
  month: number; // 0..11
  day: number;
  hour: number;
  minute: number;
}

function isoToLocal(iso: string | null): DTState | null {
  if (!iso) return null;
  const d = new Date(iso);
  if (isNaN(d.getTime())) return null;
  return {
    year: d.getFullYear(),
    month: d.getMonth(),
    day: d.getDate(),
    hour: d.getHours(),
    minute: d.getMinutes(),
  };
}

function localToIso(s: DTState): string {
  return new Date(s.year, s.month, s.day, s.hour, s.minute, 0, 0).toISOString();
}

function emptyState(): DTState {
  const t = new Date();
  return {
    year: t.getFullYear(),
    month: t.getMonth(),
    day: t.getDate(),
    hour: t.getHours(),
    minute: t.getMinutes(),
  };
}

// Draft state — отдельный от props.value. Меняется только при `Сохранить`.
const draft = ref<DTState>(isoToLocal(props.value) ?? emptyState());

watch(
  () => [props.value, open.value] as const,
  ([_iso, isOpen]) => {
    if (isOpen) {
      // При открытии — копируем актуальное value в draft (или сегодня).
      draft.value = isoToLocal(props.value) ?? emptyState();
    }
  },
);

const dateIso = computed(() => {
  return `${draft.value.year}-${pad(draft.value.month + 1)}-${pad(draft.value.day)}`;
});

function onDatePick(iso: string) {
  const [y, m, d] = iso.split("-").map(Number);
  draft.value = { ...draft.value, year: y, month: m - 1, day: d };
}

// --- text-based time input HH:MM ---
const timeInputRaw = ref("");

watch(
  () => [open.value, draft.value.hour, draft.value.minute] as const,
  ([isOpen, h, m]) => {
    if (isOpen) timeInputRaw.value = `${pad(h)}:${pad(m)}`;
  },
  { immediate: true },
);

function normalizeTimeText(text: string): { hour: number; minute: number } {
  const digits = text.replace(/\D/g, "").slice(0, 4);
  if (digits.length === 0) return { hour: 0, minute: 0 };
  let h = 0;
  let m = 0;
  if (digits.length <= 2) {
    h = Number(digits);
  } else {
    h = Number(digits.slice(0, digits.length - 2));
    m = Number(digits.slice(-2));
  }
  h = Math.max(0, Math.min(23, h));
  m = Math.max(0, Math.min(59, m));
  return { hour: h, minute: m };
}

function formatTimeForDisplay(text: string): string {
  // Пользователь печатает 1330 → показываем 13:30. Раздели по последним двум цифрам.
  const digits = text.replace(/\D/g, "").slice(0, 4);
  if (digits.length <= 2) return digits;
  return `${digits.slice(0, digits.length - 2)}:${digits.slice(-2)}`;
}

function onTimeInput(e: Event) {
  const raw = (e.target as HTMLInputElement).value;
  const formatted = formatTimeForDisplay(raw);
  timeInputRaw.value = formatted;
}

function commitTimeInput() {
  const { hour, minute } = normalizeTimeText(timeInputRaw.value);
  draft.value = { ...draft.value, hour, minute };
  timeInputRaw.value = `${pad(hour)}:${pad(minute)}`;
}

function onTimeKeyDown(e: KeyboardEvent) {
  if (e.key === "Enter") {
    e.preventDefault();
    commitTimeInput();
    applyDraft();
  }
}

const RU_MONTHS_SHORT = [
  "янв",
  "фев",
  "мар",
  "апр",
  "мая",
  "июн",
  "июл",
  "авг",
  "сен",
  "окт",
  "ноя",
  "дек",
] as const;

function refDate(): Date | null {
  const r = props.reference;
  if (r === null || r === undefined) return null;
  const d = r instanceof Date ? r : new Date(r);
  return isNaN(d.getTime()) ? null : d;
}

const displayLabel = computed(() => {
  if (!props.value) return props.placeholder;
  const s = isoToLocal(props.value)!;
  const time = `${pad(s.hour)}:${pad(s.minute)}`;
  const ref = refDate();
  if (!ref) {
    // Без референса — полный формат, как раньше.
    return `${pad(s.day)} ${RU_MONTHS_SHORT[s.month]} ${s.year}, ${time}`;
  }
  const sameYear = ref.getFullYear() === s.year;
  const sameMonth = sameYear && ref.getMonth() === s.month;
  const sameDay = sameMonth && ref.getDate() === s.day;

  if (sameDay) return time;
  if (sameMonth) return `${pad(s.day)} ${time}`;
  if (sameYear) return `${pad(s.day)}.${pad(s.month + 1)} ${time}`;
  return `${pad(s.day)}.${pad(s.month + 1)}.${String(s.year).slice(-2)} ${time}`;
});

function clearValue() {
  emit("update:value", null);
  open.value = false;
}

function applyDraft() {
  // Если фокус был в time input — нормализуем перед сохранением.
  commitTimeInput();
  emit("update:value", localToIso(draft.value));
  open.value = false;
}

function reposition() {
  const trigger = triggerRef.value;
  const panel = panelRef.value;
  if (!trigger || !panel) return;
  const rect = trigger.getBoundingClientRect();
  const panelH = panel.offsetHeight;
  const panelW = panel.offsetWidth;
  const margin = 6;
  const spaceBelow = window.innerHeight - rect.bottom - margin;
  const spaceAbove = rect.top - margin;
  const placement: "below" | "above" =
    panelH <= spaceBelow || spaceBelow >= spaceAbove ? "below" : "above";

  let top: number;
  if (placement === "below") {
    top = Math.min(rect.bottom + margin, window.innerHeight - panelH - 8);
  } else {
    top = Math.max(rect.top - panelH - margin, 8);
  }
  let left = rect.left;
  if (left + panelW > window.innerWidth - 8) {
    left = Math.max(8, window.innerWidth - panelW - 8);
  }
  panelPosition.value = { top, left, placement };
}

function onTriggerClick() {
  open.value = !open.value;
  if (open.value) {
    nextTick(() => reposition());
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

function onDocKey(e: KeyboardEvent) {
  if (e.key === "Escape" && open.value) open.value = false;
}

function onWindowResize() {
  if (open.value) reposition();
}

watch(open, (isOpen) => {
  if (isOpen) {
    document.addEventListener("pointerdown", onDocPointerDown);
    document.addEventListener("keydown", onDocKey);
    window.addEventListener("resize", onWindowResize);
    window.addEventListener("scroll", onWindowResize, true);
  } else {
    document.removeEventListener("pointerdown", onDocPointerDown);
    document.removeEventListener("keydown", onDocKey);
    window.removeEventListener("resize", onWindowResize);
    window.removeEventListener("scroll", onWindowResize, true);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointerDown);
  document.removeEventListener("keydown", onDocKey);
  window.removeEventListener("resize", onWindowResize);
  window.removeEventListener("scroll", onWindowResize, true);
});
</script>

<template>
  <div class="kosmos-dtp">
    <button
      ref="triggerRef"
      type="button"
      class="kosmos-dtp__trigger"
      :class="{ 'kosmos-dtp__trigger--placeholder': !value }"
      :aria-label="label ?? 'Выбрать дату и время'"
      @click="onTriggerClick"
    >
      <span class="kosmos-dtp__label">{{ displayLabel }}</span>
      <CalendarIcon :size="14" :stroke-width="1.7" class="kosmos-dtp__icon" />
    </button>

    <Teleport to="body">
      <div
        v-if="open"
        ref="panelRef"
        class="kosmos-dtp__panel"
        role="dialog"
        :style="{ top: panelPosition.top + 'px', left: panelPosition.left + 'px' }"
      >
        <Calendar :value="dateIso" @pick="onDatePick" />

        <div class="kosmos-dtp__time">
          <input
            id="kosmos-dtp-time-input"
            class="kosmos-dtp__time-input"
            type="text"
            inputmode="numeric"
            placeholder="HH:MM"
            maxlength="5"
            :value="timeInputRaw"
            @input="onTimeInput"
            @blur="commitTimeInput"
            @keydown="onTimeKeyDown"
          />
        </div>

        <footer class="kosmos-dtp__foot">
          <button type="button" class="kosmos-dtp__btn kosmos-dtp__btn--danger" @click="clearValue">
            Очистить
          </button>
          <div class="kosmos-dtp__foot-spacer" />
          <button type="button" class="kosmos-dtp__btn" @click="open = false">Отмена</button>
          <button
            type="button"
            class="kosmos-dtp__btn kosmos-dtp__btn--primary"
            @click="applyDraft"
          >
            Сохранить
          </button>
        </footer>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.kosmos-dtp {
  position: relative;
  display: inline-flex;
  width: 100%;
}

.kosmos-dtp__trigger {
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
  font-family: var(--font-sans, inherit);
  font-size: 0.8125rem;
  font-variant-numeric: tabular-nums;
  cursor: pointer;
  transition: border-color 160ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-dtp__trigger:hover {
  border-color: color-mix(in srgb, var(--accent) 35%, transparent);
}

.kosmos-dtp__trigger--placeholder {
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
}

.kosmos-dtp__label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.kosmos-dtp__icon {
  color: var(--foreground);
  opacity: 0.7;
  flex-shrink: 0;
}

.kosmos-dtp__panel {
  position: fixed;
  z-index: 9500;
  display: flex;
  flex-direction: column;
  min-width: 280px;
  max-width: min(420px, calc(100vw - 16px));
  background: var(--popover, var(--background));
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.85);
  corner-shape: var(--corner-shape);
  box-shadow:
    0 12px 32px rgb(0 0 0 / 28%),
    0 4px 12px rgb(0 0 0 / 14%);
  font-family: var(--font-sans, inherit);
}

.kosmos-dtp__time {
  display: flex;
  align-items: center;
  padding: 0.5rem 0.75rem;
  border-top: 1px solid var(--border);
}

.kosmos-dtp__time-input {
  flex: 1;
  height: 34px;
  text-align: center;
  padding: 0 0.625rem;
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  border: 2px solid var(--border);
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 1.0625rem;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.02em;
  outline: none;
}

.kosmos-dtp__time-input:focus {
  border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  background: var(--background);
}

.kosmos-dtp__foot {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.625rem 0.75rem;
  border-top: 1px solid var(--border);
}

.kosmos-dtp__foot-spacer {
  flex: 1;
}

.kosmos-dtp__btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 30px;
  padding: 0 0.875rem;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.6);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: inherit;
  font-size: 0.8125rem;
  font-weight: 500;
  cursor: pointer;
  transition: background-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-dtp__btn:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.kosmos-dtp__btn--primary {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--accent-foreground);
}

.kosmos-dtp__btn--primary:hover {
  background: color-mix(in srgb, var(--accent) 90%, var(--foreground));
}

.kosmos-dtp__btn--danger {
  color: var(--destructive);
  border-color: color-mix(in srgb, var(--destructive) 35%, transparent);
}

.kosmos-dtp__btn--danger:hover {
  background: color-mix(in srgb, var(--destructive) 14%, transparent);
}
</style>
