<script setup lang="ts">
import { Calendar, Inbox, Settings, Star } from "lucide-vue-next";
import DesktopChrome from "../../components/DesktopChrome.vue";
import DesktopContentSurface from "../../components/DesktopContentSurface.vue";
import Sidebar, { type SidebarNavItem } from "../../components/Sidebar.vue";
import StatusDot from "../../components/StatusDot.vue";
import TitlebarHistoryControls from "../../components/TitlebarHistoryControls.vue";

const primaryItems: SidebarNavItem[] = [
  { id: "inbox", icon: Inbox, to: "/inbox", label: "Инбокс" },
  { id: "today", icon: Calendar, to: "/today", label: "Сегодня", active: true },
  { id: "upcoming", icon: Star, to: "/upcoming", label: "Скоро" },
];
</script>

<template>
  <Story title="DesktopChrome + Surface" group="patterns" :layout="{ type: 'single', iframe: true }">
    <Variant title="Полный shell — Windows">
      <div class="frame">
        <DesktopChrome platform="windows" title="Delphi">
          <template #titlebar-trailing>
            <StatusDot tone="success" label="Подключено" />
          </template>
          <template #sidebar>
            <Sidebar :primary-items="primaryItems" :default-width="220" />
          </template>

          <DesktopContentSurface>
            <h1 class="kosmos-page-title">Сегодня</h1>
            <p>Контент-область автоматически получает 16px скругление и левую границу — провайдер `DesktopChrome` сообщил что sidebar есть.</p>
          </DesktopContentSurface>
        </DesktopChrome>
      </div>
    </Variant>

    <Variant title="macOS + history controls">
      <div class="frame">
        <DesktopChrome platform="mac" title="Eden">
          <template #titlebar-center>
            <TitlebarHistoryControls />
          </template>
          <template #titlebar-trailing>
            <Settings :size="16" />
          </template>
          <template #sidebar>
            <Sidebar :primary-items="primaryItems" :is-mac="true" :default-width="220" />
          </template>

          <DesktopContentSurface>
            <h1 class="kosmos-page-title">Заметки</h1>
            <p>macOS: titlebar получает левый safe-area под traffic lights.</p>
          </DesktopContentSurface>
        </DesktopChrome>
      </div>
    </Variant>

    <Variant title="Без sidebar — content без скругления слева">
      <div class="frame">
        <DesktopChrome platform="windows" title="Settings">
          <template #titlebar-trailing>
            <StatusDot tone="neutral" label="Idle" />
          </template>
          <DesktopContentSurface>
            <h1 class="kosmos-page-title">Настройки</h1>
            <p>
              Без слота `sidebar`: <code>DesktopContentSurface</code> через provide/inject
              понимает что левого края нет и убирает скругление + левую границу.
            </p>
          </DesktopContentSurface>
        </DesktopChrome>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.frame {
  height: 520px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  overflow: hidden;
}
code {
  font-family: var(--font-mono);
  font-size: 0.85em;
  padding: 0 0.3em;
  background: var(--secondary);
  border-radius: 4px;
}
</style>
