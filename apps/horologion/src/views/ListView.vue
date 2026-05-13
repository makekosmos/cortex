<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { Trash2 } from "lucide-vue-next";
import { ContextMenu, ContextMenuItem, useContextMenu } from "@kepler/visuals";
import type { TimeEntry } from "@shared/ipc-types";
import { formatDuration, formatDayHeader, dayKey } from "../lib/format";
import { entriesChangedAt, notifyEntriesChanged } from "../lib/store";
import EditEntryModal from "../components/EditEntryModal.vue";

interface EntryGroup {
  /** Стабильный ключ группы (используем id первой записи). */
  key: string;
  title: string;
  taskTitle: string | null;
  billable: boolean;
  entries: TimeEntry[]; // всегда ≥1
  total: number;        // суммарная длительность всех entries в секундах
}

interface DayGroup {
  day: string;
  label: string;
  total: number;
  groups: EntryGroup[];
}

const entries = ref<TimeEntry[]>([]);

async function load() {
  entries.value = await window.horologion.timeEntries.list();
}

onMounted(load);
watch(entriesChangedAt, () => {
  void load();
});

function durationSec(e: TimeEntry): number {
  const start = new Date(e.startedAt).getTime();
  const end = e.endedAt ? new Date(e.endedAt).getTime() : Date.now();
  return Math.max(0, Math.floor((end - start) / 1000));
}

/** Ключ группировки: одинаковый title (trim, lower) + одинаковый taskId + billable. */
function groupKey(e: TimeEntry): string {
  return `${e.title.trim().toLowerCase()}|${e.taskId ?? ""}|${e.billable ? "1" : "0"}`;
}

const groupedByDay = computed<DayGroup[]>(() => {
  const byDay = new Map<string, TimeEntry[]>();
  for (const e of entries.value) {
    const k = dayKey(e.startedAt);
    if (!byDay.has(k)) byDay.set(k, []);
    byDay.get(k)!.push(e);
  }
  return Array.from(byDay.entries()).map(([day, list]) => {
    const groupMap = new Map<string, EntryGroup>();
    for (const e of list) {
      const gk = groupKey(e);
      const existing = groupMap.get(gk);
      if (existing) {
        existing.entries.push(e);
        existing.total += durationSec(e);
      } else {
        groupMap.set(gk, {
          key: e.id,
          title: e.title,
          taskTitle: e.taskTitle,
          billable: e.billable,
          entries: [e],
          total: durationSec(e),
        });
      }
    }
    const groups = Array.from(groupMap.values()).sort((a, b) => {
      const aStart = new Date(a.entries[0].startedAt).getTime();
      const bStart = new Date(b.entries[0].startedAt).getTime();
      return bStart - aStart;
    });
    return {
      day,
      label: formatDayHeader(list[0].startedAt),
      total: groups.reduce((acc, g) => acc + g.total, 0),
      groups,
    };
  });
});

const todayTotal = computed(() => {
  const today = new Date().toISOString().slice(0, 10);
  const todayDay = groupedByDay.value.find((g) => g.day === today);
  return todayDay?.total ?? 0;
});

function groupIsRunning(g: EntryGroup): boolean {
  return g.entries.some((e) => !e.endedAt);
}

// --- Expand state: какие группы раскрыты (показывают индивидуальные записи) ---
const expanded = ref<Set<string>>(new Set());
function toggleExpand(key: string) {
  const next = new Set(expanded.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expanded.value = next;
}
function isExpanded(key: string): boolean {
  return expanded.value.has(key);
}

// --- Edit modal ---
const editing = ref<TimeEntry | null>(null);

function openEdit(g: EntryGroup) {
  // Для группы открываем самую свежую запись (entries уже не отсортированы внутри,
  // берём ту, у которой больше startedAt).
  const latest = g.entries.slice().sort(
    (a, b) => new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime(),
  )[0];
  editing.value = latest;
}

function closeEdit() {
  editing.value = null;
}

async function onSave(patch: {
  id: string;
  title: string;
  startedAt: string;
  endedAt: string | null;
  taskId: string | null;
  taskTitle: string | null;
}) {
  await window.horologion.timeEntries.update(patch);
  closeEdit();
  notifyEntriesChanged();
}

async function onDelete(id: string) {
  await window.horologion.timeEntries.delete(id);
  closeEdit();
  notifyEntriesChanged();
}

// --- Контекст-меню (ПКМ) на группе ---
const menu = useContextMenu<EntryGroup>();
async function deleteFromMenu() {
  const g = menu.payload.value;
  menu.close();
  if (!g) return;
  // Удаляем ВСЕ записи группы (все идентичные). Это поведение группового удаления.
  await Promise.all(g.entries.map((e) => window.horologion.timeEntries.delete(e.id)));
  notifyEntriesChanged();
}

function formatTimeOfDay(iso: string): string {
  const d = new Date(iso);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** Делит title записи на куски, выделяя `@<taskTitle>` как accent-span. */
function titleParts(g: EntryGroup): Array<{ type: "text" | "task"; text: string }> {
  if (!g.taskTitle) return [{ type: "text", text: g.title || "Без названия" }];
  const needle = `@${g.taskTitle}`;
  const idx = g.title.indexOf(needle);
  if (idx < 0) return [{ type: "text", text: g.title || "Без названия" }];
  const before = g.title.slice(0, idx);
  const after = g.title.slice(idx + needle.length);
  const parts: Array<{ type: "text" | "task"; text: string }> = [];
  if (before) parts.push({ type: "text", text: before });
  parts.push({ type: "task", text: g.taskTitle });
  if (after) parts.push({ type: "text", text: after });
  return parts;
}
</script>

<template>
  <div class="list">
    <header class="list__head">
      <span class="list__today">Сегодня</span>
      <span class="list__today-total">{{ formatDuration(todayTotal) }}</span>
    </header>

    <div v-if="entries.length === 0" class="empty">
      Записей пока нет. Введи что-то в поле выше и нажми старт.
    </div>

    <div v-else class="days">
      <div v-for="d in groupedByDay" :key="d.day" class="day">
        <header class="day__head">
          <span class="day__label">{{ d.label }}</span>
          <span class="day__total">{{ formatDuration(d.total) }}</span>
        </header>
        <ul class="day__list">
          <template v-for="g in d.groups" :key="g.key">
            <li
              class="row"
              :class="{ 'row--running': groupIsRunning(g) }"
              @click="openEdit(g)"
              @contextmenu="(ev) => menu.open(ev, g)"
            >
              <button
                v-if="g.entries.length > 1"
                type="button"
                class="row__count"
                :class="{ 'row__count--open': isExpanded(g.key) }"
                :title="isExpanded(g.key) ? 'Свернуть' : 'Раскрыть'"
                @click.stop="toggleExpand(g.key)"
              >
                {{ g.entries.length }}
              </button>
              <div class="row__title">
                <template v-for="(p, i) in titleParts(g)" :key="i">
                  <span v-if="p.type === 'task'" class="row__task">{{ p.text }}</span>
                  <template v-else>{{ p.text }}</template>
                </template>
              </div>
              <span class="row__dur">{{ formatDuration(g.total) }}</span>
            </li>

            <li
              v-if="g.entries.length > 1"
              class="row__expand-wrap"
              :class="{ 'row__expand-wrap--open': isExpanded(g.key) }"
            >
              <ul class="row__sublist">
                <li
                  v-for="e in g.entries"
                  :key="e.id"
                  class="subrow"
                  :class="{ 'subrow--running': !e.endedAt }"
                  @click="editing = e"
                >
                  <span class="subrow__time">{{ formatTimeOfDay(e.startedAt) }}</span>
                  <span class="subrow__time-sep">—</span>
                  <span class="subrow__time">{{
                    e.endedAt ? formatTimeOfDay(e.endedAt) : "сейчас"
                  }}</span>
                  <span class="subrow__spacer" />
                  <span class="subrow__dur">{{ formatDuration(durationSec(e)) }}</span>
                </li>
              </ul>
            </li>
          </template>
        </ul>
      </div>
    </div>

    <ContextMenu
      :open="menu.isOpen.value"
      :x="menu.x.value"
      :y="menu.y.value"
      @close="menu.close"
    >
      <ContextMenuItem destructive @click="deleteFromMenu">
        <Trash2 :size="14" :stroke-width="1.7" />
        Удалить {{ menu.payload.value && menu.payload.value.entries.length > 1
          ? `(${menu.payload.value.entries.length})`
          : "" }}
      </ContextMenuItem>
    </ContextMenu>

    <EditEntryModal
      :open="editing !== null"
      :entry="editing"
      @close="closeEdit"
      @save="onSave"
      @delete="onDelete"
    />
  </div>
</template>

<style scoped>
.list {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  width: 100%;
}

.list__head {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 0.5rem;
}

.list__today {
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  font-weight: 600;
}

.list__today-total {
  font-family: var(--font-mono);
  font-size: 0.9375rem;
  font-variant-numeric: tabular-nums;
  color: var(--foreground);
  font-weight: 600;
}

.empty {
  padding: 4rem 0;
  text-align: center;
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
  font-size: 0.875rem;
}

.days {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.day {
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.85);
  corner-shape: var(--corner-shape);
  overflow: hidden;
  background: color-mix(in srgb, var(--foreground) 1.5%, var(--background));
}

.day__head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.625rem 0.875rem;
  background: color-mix(in srgb, var(--foreground) 3%, transparent);
  border-bottom: 1px solid var(--border);
}

.day__label {
  font-size: 0.8125rem;
  font-weight: 600;
  color: color-mix(in srgb, var(--foreground) 75%, transparent);
}

.day__total {
  font-family: var(--font-mono);
  font-size: 0.8125rem;
  font-variant-numeric: tabular-nums;
  color: var(--foreground);
  font-weight: 500;
}

.day__list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.row {
  /* Flex без резерва — title строки-одиночки прижат к левому краю.
     В строках с badge'ем badge сидит inline перед title с gap'ом. */
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.625rem 0.875rem;
  border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  cursor: pointer;
  transition: background-color 300ms cubic-bezier(0.2, 0, 0, 1);
}

.row:first-child {
  border-top: none;
}

.row:hover {
  background: color-mix(in srgb, var(--foreground) 3.5%, transparent);
}

.row--running {
  background: color-mix(in srgb, var(--status-success) 7%, transparent);
}

.row__count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  font-size: 0.75rem;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  border: none;
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: color-mix(in srgb, var(--foreground) 75%, transparent);
  cursor: pointer;
  font-family: inherit;
  transition:
    background-color 160ms cubic-bezier(0.2, 0, 0, 1),
    color 160ms cubic-bezier(0.2, 0, 0, 1);
}

.row__count:hover {
  background: color-mix(in srgb, var(--accent) 22%, transparent);
  color: var(--accent);
}

.row__count--open {
  background: var(--accent);
  color: var(--accent-foreground);
}

.row__count--open:hover {
  background: var(--accent);
  color: var(--accent-foreground);
}

/* Expand-обёртка — CSS Grid trick для smooth height-анимации. */
.row__expand-wrap {
  display: grid;
  grid-template-rows: 0fr;
  transition: grid-template-rows 280ms cubic-bezier(0.2, 0, 0, 1);
  background: color-mix(in srgb, var(--foreground) 1.5%, transparent);
  list-style: none;
}

.row__expand-wrap--open {
  grid-template-rows: 1fr;
}

.row__sublist {
  overflow: hidden;
  list-style: none;
  margin: 0;
  padding: 0;
  min-height: 0;
}

.subrow {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.875rem 0.4rem 2.875rem;
  font-size: 0.8125rem;
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  cursor: pointer;
  transition: background-color 200ms cubic-bezier(0.2, 0, 0, 1);
  border-top: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
}

.subrow:hover {
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  color: var(--foreground);
}

.subrow--running {
  color: var(--status-success);
}

.subrow__time {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

.subrow__time-sep {
  opacity: 0.5;
}

.subrow__spacer {
  flex: 1;
}

.subrow__dur {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  font-weight: 500;
  color: var(--foreground);
}

.row__title {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 0.4rem;
  min-width: 0;
  font-size: 0.875rem;
  color: var(--foreground);
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row__task {
  color: var(--accent);
  font-weight: 600;
}

.row__bill {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  margin-left: 0.25rem;
  padding: 0 0.4rem;
  height: 18px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--horologion-billable) 22%, transparent);
  color: var(--horologion-billable-fg);
  font-size: 0.6875rem;
  font-family: var(--font-mono);
  font-weight: 700;
}

.row__dur {
  flex-shrink: 0;
  font-family: var(--font-mono);
  font-size: 0.875rem;
  font-variant-numeric: tabular-nums;
  color: var(--foreground);
  font-weight: 500;
  text-align: right;
}
</style>
