<script setup lang="ts">
import CornerLabels from "./components/CornerLabels.vue";
import Hero from "./components/Hero.vue";
// Launcher showcase временно скрыт; import оставлен для будущего возврата.
// import LauncherView from "@/views/LauncherView.vue";
</script>

<template>
  <div class="page">
    <CornerLabels />

    <main class="main">
      <Hero />
    </main>
  </div>
</template>

<style scoped>
.page {
  position: relative;
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

.main {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 60px;
  padding: 80px 24px;
  padding-top: max(120px, 14vh);
  padding-bottom: max(80px, 8vh);
}

@media (max-width: 760px) {
  .main {
    gap: 32px;
    padding-top: max(80px, 10vh);
    padding-bottom: max(40px, 5vh);
  }
}
</style>
