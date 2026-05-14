<script setup lang="ts">
// Hash-based router для dashboard окна. Один Vue app, переключение по
// `window.location.hash`. Используем listener на 'hashchange', чтобы избежать
// зависимости от vue-router (он не нужен — у нас только 2 экрана).
import { computed, onMounted, onUnmounted, ref } from "vue";
import DashboardWelcomeView from "./DashboardWelcomeView.vue";
import DashboardSpaceView from "./DashboardSpaceView.vue";

const currentHash = ref(window.location.hash);

function onHashChange(): void {
  currentHash.value = window.location.hash;
}

onMounted(() => {
  window.addEventListener("hashchange", onHashChange);
});
onUnmounted(() => {
  window.removeEventListener("hashchange", onHashChange);
});

const route = computed(() => {
  const hash = currentHash.value;
  const match = hash.match(/^#\/dashboard\/space\/([^/]+)/);
  if (match) {
    return { name: "space" as const, spaceId: decodeURIComponent(match[1]) };
  }
  return { name: "welcome" as const };
});
</script>

<template>
  <DashboardSpaceView
    v-if="route.name === 'space'"
    :space-id="route.spaceId"
  />
  <DashboardWelcomeView v-else />
</template>
