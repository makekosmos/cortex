<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from "vue";
import type { SearchResult } from "@shared/ipc-types";

const query = ref("");
const results = ref<SearchResult[]>([]);
const inputRef = ref<HTMLInputElement | null>(null);

async function onInput() {
  if (!query.value.trim()) {
    results.value = [];
    return;
  }
  // Phase 1 stub: kepler-backend search wiring — Phase 2.
  try {
    results.value = await window.kepler.search.query(query.value);
  } catch (e) {
    console.warn("search failed", e);
    results.value = [];
  }
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    void window.kepler.window.hide();
  }
}

let offShow = () => {};

onMounted(() => {
  offShow = window.kepler.window.onShow(() => {
    query.value = "";
    results.value = [];
    void nextTick(() => inputRef.value?.focus());
  });
  void nextTick(() => inputRef.value?.focus());
});

onUnmounted(() => offShow());
</script>

<template>
  <div class="launcher" @keydown="onKey">
    <input
      ref="inputRef"
      v-model="query"
      class="search"
      type="text"
      placeholder="Поиск, команды, быстрая запись…"
      spellcheck="false"
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      @input="onInput"
    />
    <ul v-if="results.length > 0" class="results">
      <li v-for="r in results" :key="r.id" class="result">
        <span class="title">{{ r.title }}</span>
        <span class="type">{{ r.type_id }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.launcher {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: color-mix(in srgb, var(--background) 88%, transparent);
  backdrop-filter: blur(20px) saturate(140%);
  -webkit-backdrop-filter: blur(20px) saturate(140%);
  border: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  border-radius: 14px;
  overflow: hidden;
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
}

.search::placeholder {
  color: color-mix(in srgb, var(--foreground) 36%, transparent);
}

.results {
  flex: 1;
  margin: 0;
  padding: 6px;
  list-style: none;
  overflow-y: auto;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 6%, transparent);
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
  background: color-mix(in srgb, var(--accent) 14%, transparent);
}

.title {
  color: var(--foreground);
  font-size: 14px;
}

.type {
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  font-size: 12px;
  font-family: var(--font-mono);
}
</style>
