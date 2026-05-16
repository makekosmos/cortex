<script setup lang="ts">
import { ref } from "vue";
import { Copy, Trash2, Pencil, FolderOpen } from "lucide-vue-next";
import ContextMenu from "../../components/ContextMenu.vue";
import ContextMenuItem from "../../components/ContextMenuItem.vue";
import { useContextMenu } from "../../composables/useContextMenu";

interface Task {
  id: string;
  title: string;
}

const tasks: Task[] = [
  { id: "1", title: "Пописать code-review на PR #42" },
  { id: "2", title: "Подумать над структурой sync events" },
  { id: "3", title: "Обновить smoke-matrix после Phase 8b" },
];

const menu = useContextMenu<Task>();
const action = ref<string | null>(null);

function emit(name: string, payload?: Task) {
  action.value = payload ? `${name} → ${payload.title}` : name;
  menu.close();
}
</script>

<template>
  <Story title="ContextMenu" group="popovers" :layout="{ type: 'single', iframe: true }">
    <Variant title="Контекст-меню над списком">
      <div class="story-canvas">
        <p class="story-label">ПКМ по строке. Хелпер `useContextMenu&lt;T&gt;()` — типизированный payload.</p>
        <ul class="rows">
          <li
            v-for="t in tasks"
            :key="t.id"
            class="row"
            @contextmenu.prevent="(e) => menu.open(e, t)"
          >
            {{ t.title }}
          </li>
        </ul>

        <ContextMenu
          :open="menu.isOpen.value"
          :x="menu.x.value"
          :y="menu.y.value"
          @close="menu.close"
        >
          <ContextMenuItem @click="emit('Открыть', menu.payload.value ?? undefined)">
            <FolderOpen :size="14" /> Открыть
          </ContextMenuItem>
          <ContextMenuItem @click="emit('Редактировать', menu.payload.value ?? undefined)">
            <Pencil :size="14" /> Редактировать
          </ContextMenuItem>
          <ContextMenuItem @click="emit('Копировать', menu.payload.value ?? undefined)">
            <Copy :size="14" /> Копировать
          </ContextMenuItem>
          <ContextMenuItem
            :destructive="true"
            @click="emit('Удалить', menu.payload.value ?? undefined)"
          >
            <Trash2 :size="14" /> Удалить
          </ContextMenuItem>
        </ContextMenu>

        <p class="muted">Last action: <code>{{ action ?? "—" }}</code></p>
      </div>
    </Variant>

    <Variant title="Items: disabled & destructive">
      <div class="story-canvas">
        <p class="story-label">
          Item варианты — изолированные ContextMenuItem без триггера.
        </p>
        <div class="menu-frame">
          <ContextMenuItem>Обычный пункт</ContextMenuItem>
          <ContextMenuItem :disabled="true">Disabled (нельзя)</ContextMenuItem>
          <ContextMenuItem :destructive="true">Опасный пункт</ContextMenuItem>
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.rows {
  list-style: none;
  padding: 0;
  margin: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  overflow: hidden;
  max-width: 480px;
}
.row {
  padding: 0.65rem 0.85rem;
  border-bottom: 1px solid var(--border);
  cursor: context-menu;
}
.row:last-child {
  border-bottom: none;
}
.row:hover {
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
}
.menu-frame {
  display: flex;
  flex-direction: column;
  width: 220px;
  padding: 0.25rem;
  background: var(--popover);
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.85);
}
.muted {
  margin-top: 1rem;
  font-size: 0.8rem;
  color: var(--muted-foreground);
}
code {
  font-family: var(--font-mono);
}
</style>
