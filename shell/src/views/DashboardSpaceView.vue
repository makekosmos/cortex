<script setup lang="ts">
// C2 stub. Полноценная реализация sidebar + object table — в C4.
defineProps<{
  spaceId: string;
}>();

function goBack(): void {
  window.location.hash = "#/dashboard/welcome";
}
</script>

<template>
  <div class="stub">
    <button class="back" type="button" @click="goBack">← К spaces</button>
    <div class="msg">Space {{ spaceId }} — в разработке.</div>
  </div>
</template>

<style scoped>
.stub {
  width: 100%;
  height: 100vh;
  background: var(--background);
  color: var(--foreground);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
}

.back {
  background: transparent;
  border: 1px solid var(--border);
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
  border-radius: 6px;
  padding: 6px 12px;
  font-size: 12px;
  cursor: pointer;
}

.msg {
  font-size: 14px;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
}
</style>
