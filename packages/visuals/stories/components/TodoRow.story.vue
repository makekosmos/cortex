<script setup lang="ts">
import { reactive, ref } from "vue";
import TodoRow, { type TodoRowItem, type TodoRowUpdate } from "../../components/TodoRow.vue";

const single = reactive<TodoRowItem>({
  id: "t-1",
  title: "Записать обзор feedback с команды",
  notes: "ПКМ — context menu. Click — раскрыть редактор.",
  scheduledDate: "2026-05-16",
});

const completed = reactive<TodoRowItem>({
  id: "t-2",
  title: "Закрыть PR с migration to extension store",
  isCompleted: true,
});

const billable = reactive<TodoRowItem>({
  id: "t-3",
  title: "Дизайн консультация — Acme Co.",
  scheduledDate: "2026-05-20",
  billable: true,
  price: 12000,
});

const list = reactive<TodoRowItem[]>([
  { id: "l-1", title: "Прогнать `bun run ark:smoke`" },
  { id: "l-2", title: "Обновить STATUS.md после релиза 0.1.7" },
  { id: "l-3", title: "Разобрать backlog Horologion", scheduledDate: "2026-05-18" },
  { id: "l-4", title: "Закрыть PR autoupdater banner", isCompleted: true },
]);

const lastEvent = ref<string>("");

function applyUpdate(target: TodoRowItem, patch: TodoRowUpdate) {
  Object.assign(target, patch);
  lastEvent.value = `update(${target.id}) ${JSON.stringify(patch)}`;
}

function trash(target: TodoRowItem) {
  target.isTrashed = true;
  lastEvent.value = `trash(${target.id})`;
}

function complete(target: TodoRowItem) {
  target.isCompleted = !target.isCompleted;
  lastEvent.value = `complete(${target.id}) → ${target.isCompleted}`;
}
</script>

<template>
  <Story title="TodoRow" group="complex" :layout="{ type: 'single', iframe: true }">
    <Variant title="Состояния">
      <div class="story-canvas">
        <p class="story-label">
          Click — раскрывает inline-редактор. ПКМ — context menu. Hover на левый край —
          checkbox + drag handle.
        </p>
        <div class="rows">
          <TodoRow
            :todo="single"
            @complete="complete(single)"
            @trash="trash(single)"
            @update="applyUpdate(single, $event)"
          />
          <TodoRow
            :todo="completed"
            @complete="complete(completed)"
            @trash="trash(completed)"
            @update="applyUpdate(completed, $event)"
          />
          <TodoRow
            :todo="billable"
            @complete="complete(billable)"
            @trash="trash(billable)"
            @update="applyUpdate(billable, $event)"
          />
        </div>
        <p class="muted">{{ lastEvent }}</p>
      </div>
    </Variant>

    <Variant title="Список с drag & drop">
      <div class="story-canvas">
        <p class="story-label">
          Drag любую строку (порог 4px). Ghost-плейсхолдер показывает место сброса.
        </p>
        <div class="rows" data-todo-list>
          <TodoRow
            v-for="t in list"
            :key="t.id"
            :todo="t"
            @complete="complete(t)"
            @trash="trash(t)"
            @update="applyUpdate(t, $event)"
            @drop="lastEvent = `drop(${t.id} → ${$event.targetId}, after=${$event.after})`"
          />
        </div>
        <p class="muted">{{ lastEvent }}</p>
      </div>
    </Variant>

    <Variant title="Read-only">
      <div class="story-canvas">
        <p class="story-label">`editable=false` — клики игнорируются.</p>
        <div class="rows">
          <TodoRow :todo="single" :draggable="false" :editable="false" />
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.rows {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
  max-width: 560px;
}
.muted {
  margin-top: 0.85rem;
  font-size: 0.78rem;
  color: var(--muted-foreground);
  font-family: var(--font-mono);
}
</style>
