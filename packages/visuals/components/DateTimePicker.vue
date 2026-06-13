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
  /** Date-only mode: hides time controls and emits selected day at local midnight. */
  dateOnly?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: "Выбрать…",
  label: undefined,
  reference: null,
  dateOnly: false,
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

function formatIsoYear(year: number): string {
  if (year < 0) return `-${String(Math.abs(year)).padStart(6, "0")}`;
  return String(year).padStart(4, "0");
}

function formatDisplayYear(year: number): string {
  return year < 0 ? `${Math.abs(year)} до н.э.` : `${year} года`;
}

function localDateTime(s: DTState): Date {
  const d = new Date(0, s.month, s.day, s.hour, s.minute, 0, 0);
  d.setFullYear(s.year);
  return d;
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
  return localDateTime(s).toISOString();
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
  return `${formatIsoYear(draft.value.year)}-${pad(draft.value.month + 1)}-${pad(draft.value.day)}`;
});

function parseDateOnlyIso(iso: string): { year: number; month: number; day: number } | null {
  const match = /^([+-]?\d{4,6})-(\d{2})-(\d{2})$/.exec(iso);
  if (!match) return null;
  return {
    year: Number(match[1]),
    month: Number(match[2]) - 1,
    day: Number(match[3]),
  };
}

function onDatePick(iso: string) {
  const parsed = parseDateOnlyIso(iso);
  if (!parsed) return;

  const next = { ...draft.value, ...parsed };
  draft.value = next;

  if (props.dateOnly) {
    emit("update:value", localToIso({ ...next, hour: 0, minute: 0 }));
    open.value = false;
  }
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
const RU_MONTHS_FULL_GENITIVE = [
  "января",
  "февраля",
  "марта",
  "апреля",
  "мая",
  "июня",
  "июля",
  "августа",
  "сентября",
  "октября",
  "ноября",
  "декабря",
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
  if (props.dateOnly) {
    return `${pad(s.day)} ${RU_MONTHS_FULL_GENITIVE[s.month]}, ${formatDisplayYear(s.year)}`;
  }
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
  const margin = 8;
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
    if (t instanceof Element && t.closest(".kosmos-dd__panel")) return;
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
  <div class="relative inline-flex w-full">
    <button
      ref="triggerRef"
      type="button"
      class="flex h-8 w-full items-center justify-between gap-2 rounded-lg border-2 border-[var(--border)] bg-[color-mix(in_srgb,var(--foreground)_4%,var(--background))] px-2 font-[var(--font-sans,inherit)] text-[0.8125rem] tabular-nums text-[var(--foreground)] transition-colors duration-150 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] hover:border-[color-mix(in_srgb,var(--accent)_35%,transparent)]"
      :class="{ 'text-[color-mix(in_srgb,var(--foreground)_50%,transparent)]': !value }"
      :aria-label="label ?? 'Выбрать дату и время'"
      @click="onTriggerClick"
    >
      <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{{
        displayLabel
      }}</span>
      <CalendarIcon
        :size="14"
        :stroke-width="1.7"
        class="shrink-0 text-[var(--foreground)] opacity-70"
      />
    </button>

    <Teleport to="body">
      <div
        v-if="open"
        ref="panelRef"
        class="fixed z-[9500] flex min-w-72 max-w-[min(416px,calc(100vw-16px))] flex-col rounded-xl border border-[var(--border)] bg-[var(--popover,var(--background))] font-[var(--font-sans,inherit)] shadow-[var(--shadow-floating)] [corner-shape:var(--corner-shape)]"
        role="dialog"
        :style="{ top: panelPosition.top + 'px', left: panelPosition.left + 'px' }"
      >
        <Calendar :value="dateIso" @pick="onDatePick" />

        <div v-if="!dateOnly" class="flex items-center border-t border-[var(--border)] px-4 py-2">
          <input
            id="kosmos-dtp-time-input"
            class="h-8 flex-1 rounded-lg border-2 border-[var(--border)] bg-[color-mix(in_srgb,var(--foreground)_4%,var(--background))] px-2 text-center font-[var(--font-mono,ui-monospace,monospace)] text-[1.0625rem] font-semibold tracking-[0.02em] text-[var(--foreground)] tabular-nums outline-none [corner-shape:var(--corner-shape)] focus:border-[color-mix(in_srgb,var(--accent)_55%,transparent)] focus:bg-[var(--background)]"
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

        <footer
          v-if="!dateOnly"
          class="flex items-center gap-2 border-t border-[var(--border)] px-4 py-2"
        >
          <button
            type="button"
            class="inline-flex h-8 items-center justify-center rounded-lg border border-[color-mix(in_srgb,var(--destructive)_35%,transparent)] bg-transparent px-4 font-[inherit] text-[0.8125rem] font-medium text-[var(--destructive)] transition-colors duration-150 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] hover:bg-[color-mix(in_srgb,var(--destructive)_14%,transparent)]"
            @click="clearValue"
          >
            Очистить
          </button>
          <div class="flex-1" />
          <button
            type="button"
            class="inline-flex h-8 items-center justify-center rounded-lg border border-[var(--border)] bg-transparent px-4 font-[inherit] text-[0.8125rem] font-medium text-[var(--foreground)] transition-colors duration-150 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)]"
            @click="open = false"
          >
            Отмена
          </button>
          <button
            type="button"
            class="inline-flex h-8 items-center justify-center rounded-lg border border-[var(--accent)] bg-[var(--accent)] px-4 font-[inherit] text-[0.8125rem] font-medium text-[var(--accent-foreground)] transition-colors duration-150 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] hover:bg-[color-mix(in_srgb,var(--accent)_90%,var(--foreground))]"
            @click="applyDraft"
          >
            Сохранить
          </button>
        </footer>
      </div>
    </Teleport>
  </div>
</template>
