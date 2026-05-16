<script setup lang="ts">
import { ref } from "vue";
import { Calendar, Inbox, Settings } from "lucide-vue-next";
import SidebarButton from "../../components/SidebarButton.vue";

const clicks = ref(0);
</script>

<template>
  <Story title="SidebarButton" group="patterns">
    <Variant title="Как кнопка (без routing)">
      <div class="story-canvas" style="background:var(--sidebar-bg);max-width:240px;">
        <SidebarButton :icon="Inbox" label="Инбокс" @click="clicks++" />
        <SidebarButton :icon="Calendar" label="Сегодня" :active="true" />
        <SidebarButton :icon="Settings" label="Настройки" />
        <p class="muted">Кликов: {{ clicks }}</p>
      </div>
    </Variant>

    <Variant title="Как RouterLink">
      <div class="story-canvas" style="background:var(--sidebar-bg);max-width:240px;">
        <SidebarButton :icon="Inbox" to="/inbox" label="Инбокс" />
        <SidebarButton :icon="Calendar" to="/today" label="Сегодня" :active="true" />
        <SidebarButton :icon="Settings" to="/settings" label="Настройки" />
      </div>
    </Variant>

    <Variant title="Только иконка (без label)">
      <div class="story-canvas" style="background:var(--sidebar-bg);width:64px;">
        <SidebarButton :icon="Inbox" />
        <SidebarButton :icon="Calendar" :active="true" />
        <SidebarButton :icon="Settings" />
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.muted {
  margin-top: 0.75rem;
  font-size: 0.8rem;
  color: var(--muted-foreground);
}
</style>
