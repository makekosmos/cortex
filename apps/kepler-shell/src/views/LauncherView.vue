<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted, nextTick } from "vue";
import type { CommandRecord } from "@shared/ipc-types";

const query = ref("");
const commands = ref<CommandRecord[]>([]);
const selectedIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
const listRef = ref<HTMLDivElement | null>(null);

interface ScoredCommand {
  cmd: CommandRecord;
  score: number;
}

function scoreCommand(cmd: CommandRecord, q: string): number {
  if (!q) return 0;
  const ql = q.toLowerCase();
  const t = cmd.title.toLowerCase();
  const s = (cmd.subtitle ?? "").toLowerCase();
  const titleIdx = t.indexOf(ql);
  const subIdx = s.indexOf(ql);
  if (titleIdx < 0 && subIdx < 0) return -1;
  // Раньше = выше; title match лучше subtitle match.
  if (titleIdx === 0) return 1000;
  if (titleIdx > 0) return 500 - titleIdx;
  return 100 - subIdx;
}

const filtered = computed<CommandRecord[]>(() => {
  const q = query.value.trim();
  if (!q) return commands.value;
  return commands.value
    .map<ScoredCommand>((cmd) => ({ cmd, score: scoreCommand(cmd, q) }))
    .filter((x) => x.score >= 0)
    .sort((a, b) => b.score - a.score)
    .map((x) => x.cmd);
});

const groupedNoQuery = computed(() => {
  if (query.value.trim()) return null;
  const actions: CommandRecord[] = [];
  const opens: CommandRecord[] = [];
  for (const c of commands.value) {
    if (c.category === "action") actions.push(c);
    else opens.push(c);
  }
  return { actions, opens };
});

function onInput() {
  selectedIndex.value = 0;
}

function flatList(): CommandRecord[] {
  if (groupedNoQuery.value) {
    return [...groupedNoQuery.value.actions, ...groupedNoQuery.value.opens];
  }
  return filtered.value;
}

async function invokeSelected() {
  const list = flatList();
  const target = list[selectedIndex.value];
  if (!target) return;
  await window.kepler.commands.invoke(target.id);
  query.value = "";
  selectedIndex.value = 0;
}

function moveSelection(delta: number) {
  const list = flatList();
  if (list.length === 0) return;
  const n = list.length;
  selectedIndex.value = (selectedIndex.value + delta + n) % n;
  void nextTick(() => {
    const el = listRef.value?.querySelector<HTMLElement>(".result.selected");
    el?.scrollIntoView({ block: "nearest" });
  });
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    void window.kepler.window.hide();
  } else if (e.key === "ArrowDown") {
    e.preventDefault();
    moveSelection(1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    moveSelection(-1);
  } else if (e.key === "Enter") {
    e.preventDefault();
    void invokeSelected();
  }
}

async function refreshCommands() {
  try {
    commands.value = await window.kepler.commands.list();
  } catch (e) {
    console.warn("commands.list failed", e);
    commands.value = [];
  }
}

let offShow = () => {};
let offCommandsUpdated = () => {};

onMounted(() => {
  offShow = window.kepler.window.onShow(() => {
    query.value = "";
    selectedIndex.value = 0;
    void refreshCommands();
    void nextTick(() => inputRef.value?.focus());
  });
  offCommandsUpdated = window.kepler.commands.onUpdated(() => {
    void refreshCommands();
  });
  void refreshCommands();
  void nextTick(() => inputRef.value?.focus());
});

onUnmounted(() => {
  offShow();
  offCommandsUpdated();
});

function indexInFlat(cmd: CommandRecord): number {
  return flatList().findIndex((c) => c.id === cmd.id);
}
</script>

<template>
  <div class="launcher" @keydown="onKey">
    <input
      ref="inputRef"
      v-model="query"
      class="search"
      type="text"
      placeholder="Поиск команд: pomo, заметка, открыть delphi…"
      spellcheck="false"
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      @input="onInput"
    />
    <div ref="listRef" class="list">
      <template v-if="groupedNoQuery">
        <template v-if="groupedNoQuery.actions.length > 0">
          <div class="section-label">Действия</div>
          <ul class="results">
            <li
              v-for="cmd in groupedNoQuery.actions"
              :key="cmd.id"
              class="result"
              :class="{ selected: indexInFlat(cmd) === selectedIndex }"
              @click="() => { selectedIndex = indexInFlat(cmd); void invokeSelected(); }"
            >
              <span class="title">{{ cmd.title }}</span>
              <span class="subtitle">{{ cmd.subtitle }}</span>
            </li>
          </ul>
        </template>
        <template v-if="groupedNoQuery.opens.length > 0">
          <div class="section-label">Открыть приложение</div>
          <ul class="results">
            <li
              v-for="cmd in groupedNoQuery.opens"
              :key="cmd.id"
              class="result"
              :class="{ selected: indexInFlat(cmd) === selectedIndex }"
              @click="() => { selectedIndex = indexInFlat(cmd); void invokeSelected(); }"
            >
              <span class="title">{{ cmd.title }}</span>
              <span class="subtitle">{{ cmd.subtitle }}</span>
            </li>
          </ul>
        </template>
      </template>
      <template v-else>
        <div v-if="filtered.length === 0" class="empty">Ничего не найдено</div>
        <ul v-else class="results">
          <li
            v-for="(cmd, idx) in filtered"
            :key="cmd.id"
            class="result"
            :class="{ selected: idx === selectedIndex }"
            @click="() => { selectedIndex = idx; void invokeSelected(); }"
          >
            <span class="title">{{ cmd.title }}</span>
            <span class="subtitle">{{ cmd.subtitle }}</span>
          </li>
        </ul>
      </template>
    </div>
  </div>
</template>

<style scoped>
.launcher {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: color-mix(in srgb, oklch(0.04 0 0) 75%, transparent);
}

.search {
  width: 100%;
  height: 64px;
  padding: 0 22px;
  border: none;
  outline: none;
  background: transparent;
  color: var(--foreground);
  font-size: 18px;
  font-weight: 400;
  flex-shrink: 0;
}

.search::placeholder {
  color: color-mix(in srgb, var(--foreground) 36%, transparent);
}

.list {
  flex: 1;
  overflow-y: auto;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
}

.section-label {
  padding: 12px 22px 6px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
}

.results {
  margin: 0;
  padding: 0 6px 8px;
  list-style: none;
}

.result {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 14px;
  border-radius: 8px;
  cursor: pointer;
}

.result:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.result.selected {
  background: color-mix(in srgb, var(--foreground) 14%, transparent);
}

.title {
  color: var(--foreground);
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.subtitle {
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  font-size: 12px;
  flex-shrink: 0;
  margin-left: 12px;
}

.empty {
  padding: 32px 22px;
  text-align: center;
  color: color-mix(in srgb, var(--foreground) 40%, transparent);
  font-size: 13px;
}
</style>
