<script setup lang="ts">
interface Props {
  title?: string;
}

defineProps<Props>();
</script>

<template>
  <aside class="kosmos-settings-sidebar">
    <div v-if="title" class="kosmos-settings-sidebar__title">
      {{ title }}
    </div>
    <div class="kosmos-settings-sidebar__content">
      <slot />
    </div>
  </aside>
</template>

<style scoped>
.kosmos-settings-sidebar {
  display: flex;
  width: 229px;
  min-width: 229px;
  height: 100%;
  box-sizing: border-box;
  flex-direction: column;
  gap: 18px;
  padding: 8px;
  border-right: 1px solid var(--border-color-strong);
  background: var(--main-background-color);
  color: #fff;
}

.kosmos-settings-sidebar__title {
  padding: 4px 6px 0;
  color: #fff;
  font-family: var(--font-sans);
  font-size: 13px;
  line-height: 1.4;
  font-weight: 500;
  -webkit-app-region: drag;
}

.kosmos-settings-sidebar__content {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 26px;
  -webkit-app-region: no-drag;
}
</style>
