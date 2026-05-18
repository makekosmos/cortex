<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from "vue";
import { Play, Square } from "lucide-vue-next";
import type { TimeEntry } from "../types";
import { entriesChangedAt, notifyEntriesChanged, pomodoroDraft } from "../lib/store";

const running = ref<TimeEntry | null>(null);
const elapsedSec = ref(0);
let tickHandle: ReturnType<typeof setInterval> | null = null;

async function refreshRunning() {
    // source: "manual" — иначе StopwatchView подхватит pomodoro_break entry
    // (созданную usePomodoroSession при trackBreaksAsRest=true) и кнопка «Стоп»
    // её закроет в обход pomodoro lifecycle. pomodoro session не узнает, что
    // entry уже не running, и при следующем pause/phase_changed второй stopTimer
    // пойдёт на уже удалённый id.
    const list = await window.horologion.timeEntries.listRunning({ source: "manual" });
    // listRunning может вернуть orphan entries без `startedAt` (или с уже
    // выставленным `endedAt` из-за рассогласованного состояния БД). Такие
    // записи ломают tick: `new Date("").getTime()` → NaN → таймер рисует
    // «NaN:NaN:NaN», а кнопка «Стоп» не может остановить запись (id есть, но
    // visual state ломается на каждом тике). Фильтруем перед выбором.
    const valid = list.filter((e) => Boolean(e.startedAt) && !e.endedAt);
    running.value = valid[0] ?? null;
    if (running.value) startTick();
    else {
        stopTick();
        elapsedSec.value = 0;
    }
}

function startTick() {
    if (tickHandle) return;
    tickHandle = setInterval(() => {
        if (!running.value) return;
        const start = new Date(running.value.startedAt).getTime();
        elapsedSec.value = Math.floor((Date.now() - start) / 1000);
    }, 1000);
    if (running.value) {
        const start = new Date(running.value.startedAt).getTime();
        elapsedSec.value = Math.floor((Date.now() - start) / 1000);
    }
}

function stopTick() {
    if (tickHandle) {
        clearInterval(tickHandle);
        tickHandle = null;
    }
}

onMounted(refreshRunning);
onBeforeUnmount(stopTick);
watch(entriesChangedAt, refreshRunning);

function pad(n: number): string {
    return String(n).padStart(2, "0");
}

const timeLabel = computed(() => {
    const s = elapsedSec.value;
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    return `${pad(h)}:${pad(m)}:${pad(sec)}`;
});

async function toggle() {
    if (running.value) {
        await window.horologion.timeEntries.stopTimer(running.value.id);
        running.value = null;
        stopTick();
        elapsedSec.value = 0;
    } else {
        const draft = pomodoroDraft.value;
        const firstTask = draft.tasks[0];
        const entry = await window.horologion.timeEntries.startTimer({
            title: draft.title.trim() || "Без названия",
            taskId: firstTask?.id ?? null,
            taskTitle: firstTask?.title ?? null,
        });
        running.value = entry;
        startTick();
    }
    notifyEntriesChanged();
}

const isRunning = computed(() => running.value !== null);
</script>

<template>
    <div class="sw">
        <span class="sw__time">{{ timeLabel }}</span>
        <span class="sw__hint">секундомер</span>

        <div class="sw__actions">
            <button class="sw__primary" :class="{ 'sw__primary--running': isRunning }" @click="toggle">
                <Square v-if="isRunning" :size="16" :stroke-width="2.2" />
                <Play v-else :size="16" :stroke-width="2.2" />
                {{ isRunning ? "Стоп" : "Начать сессию" }}
            </button>
        </div>
    </div>
</template>

<style scoped>
.sw {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    padding: 2rem 0;
}

.sw__time {
    font-family: var(--font-mono);
    font-size: 3.25rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--foreground);
    letter-spacing: -0.02em;
    line-height: 1;
}

.sw__hint {
    font-size: 0.8125rem;
    color: color-mix(in srgb, var(--foreground) 55%, transparent);
    margin-bottom: 1rem;
}

.sw__actions {
    width: 100%;
    max-width: 280px;
}

.sw__primary {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    width: 100%;
    height: 40px;
    border: none;
    border-radius: var(--radius-button);
    corner-shape: var(--corner-shape);
    background: var(--accent);
    color: var(--accent-foreground);
    font-family: inherit;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition:
        background-color 350ms cubic-bezier(0.2, 0, 0, 1),
        color 350ms cubic-bezier(0.2, 0, 0, 1);
}

.sw__primary:hover {
    background: var(--foreground);
    color: var(--background);
}

.sw__primary--running {
    background: var(--destructive);
    color: var(--accent-foreground);
}

.sw__primary--running:hover {
    background: var(--foreground);
    color: var(--background);
}
</style>
