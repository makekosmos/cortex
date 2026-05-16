<script setup lang="ts">
import { ref } from "vue";
import { Calendar, Clock, Inbox, Settings } from "lucide-vue-next";
import CommandPalette from "../../components/CommandPalette.vue";

const open = ref(true);
const query = ref("");

const allCommands = [
  { id: "today", icon: Calendar, label: "Открыть «Сегодня»" },
  { id: "inbox", icon: Inbox, label: "Открыть Инбокс" },
  { id: "pomodoro", icon: Clock, label: "Начать pomodoro:25" },
  { id: "settings", icon: Settings, label: "Открыть настройки" },
];
</script>

<template>
  <Story title="CommandPalette" group="popovers" :layout="{ type: 'single', iframe: true }">
    <Variant title="Открытая палитра">
      <div class="story-canvas story-canvas--fullheight">
        <p class="story-label">
          CommandPalette — overlay с поиском. Контент списка передаётся через default-slot.
          В реальном Kepler items приходят из command bus.
        </p>
        <button class="open-btn" type="button" @click="open = true">Открыть снова</button>
        <CommandPalette
          :open="open"
          :query="query"
          placeholder="Поиск команд…"
          @update:open="open = $event"
          @update:query="query = $event"
        >
          <ul class="results">
            <li v-for="c in allCommands" :key="c.id" class="result">
              <component :is="c.icon" :size="16" />
              <span>{{ c.label }}</span>
            </li>
          </ul>
        </CommandPalette>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.open-btn {
  padding: 0.5rem 1rem;
  border-radius: var(--radius-button);
  background: var(--accent);
  color: var(--accent-foreground);
  border: none;
  font-weight: 600;
  cursor: pointer;
}
.results {
  list-style: none;
  padding: 0.5rem;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}
.result {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  padding: 0.55rem 0.65rem;
  border-radius: calc(var(--radius) * 0.7);
  color: var(--foreground);
}
.result:hover {
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
}
</style>
