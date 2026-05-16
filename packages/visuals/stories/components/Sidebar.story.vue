<script setup lang="ts">
import { ref } from "vue";
import {
  Calendar,
  CheckSquare,
  Inbox,
  Plus,
  Settings,
  Star,
  Trash2,
} from "lucide-vue-next";
import Sidebar, {
  type SidebarConfig,
  type SidebarNavItem,
  type SidebarProjectGroup,
} from "../../components/Sidebar.vue";

const primaryItems: SidebarNavItem[] = [
  { id: "inbox", icon: Inbox, to: "/inbox", label: "Инбокс" },
  { id: "today", icon: Calendar, to: "/today", label: "Сегодня", active: true },
  { id: "upcoming", icon: Star, to: "/upcoming", label: "Скоро" },
  { id: "anytime", icon: CheckSquare, to: "/anytime", label: "В любое время" },
];

const footerItems: SidebarNavItem[] = [
  { id: "trash", icon: Trash2, to: "/trash", label: "Корзина" },
  { id: "settings", icon: Settings, to: "/settings", label: "Настройки" },
];

const projectGroups: SidebarProjectGroup[] = [
  {
    id: "active",
    label: "Активные",
    actionIcon: Plus,
    actionLabel: "Новый проект",
    onAction: () => alert("Создать проект"),
    items: [
      { id: "p-1", label: "Eden — journal", to: "/projects/eden", colorClass: "kosmos-color-today" },
      { id: "p-2", label: "Kepler shell", to: "/projects/kepler" },
      { id: "p-3", label: "Acme Co.", to: "/projects/acme", color: "#a855f7" },
    ],
  },
  {
    id: "archive",
    label: "Архив",
    defaultCollapsed: true,
    items: [
      { id: "a-1", label: "Старый PoC", to: "/projects/old" },
    ],
  },
];

const config = ref<SidebarConfig>({ width: 240, hidden: false });
</script>

<template>
  <Story title="Sidebar" group="patterns" :layout="{ type: 'single', iframe: true }">
    <Variant title="Полный (primary + projects + footer)">
      <div class="frame">
        <Sidebar
          :primary-items="primaryItems"
          :project-groups="projectGroups"
          :footer-items="footerItems"
          :initial-config="config"
          :default-width="240"
          @config-change="config = $event"
        />
        <div class="content">
          <h2>Content area</h2>
          <p class="muted">
            Sidebar шириной {{ config.width }}px, hidden = {{ config.hidden }}.
            Можно тащить за правую границу — реалтайм resize. ⌘/Ctrl+B — скрыть.
          </p>
        </div>
      </div>
    </Variant>

    <Variant title="Только primary">
      <div class="frame">
        <Sidebar :primary-items="primaryItems" />
        <div class="content">
          <p class="muted">Без проектов и footer-секций.</p>
        </div>
      </div>
    </Variant>

    <Variant title="Hidden state">
      <div class="frame">
        <Sidebar
          :primary-items="primaryItems"
          :hidden="true"
        />
        <div class="content">
          <p class="muted">Sidebar скрыт через prop `hidden=true`.</p>
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.frame {
  display: flex;
  height: 480px;
  background: var(--background);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  overflow: hidden;
}
.content {
  flex: 1;
  padding: 2rem;
  background: var(--background);
}
.muted {
  color: var(--muted-foreground);
}
</style>
