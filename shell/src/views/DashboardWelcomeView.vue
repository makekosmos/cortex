<script setup lang="ts">
import { onMounted } from "vue";
import KosmosLogo from "../dashboard/KosmosLogo.vue";
import SpaceCard from "../dashboard/SpaceCard.vue";
import { loadSpaces, spaces, spacesError, spacesLoading } from "../dashboard/store";

onMounted(() => {
  void loadSpaces();
});

function openSpace(id: string): void {
  window.location.hash = `#/dashboard/space/${id}`;
}
</script>

<template>
  <div class="welcome">
    <header class="title-stack">
      <KosmosLogo :size="64" />
      <div class="brand">Kosmos</div>
    </header>

    <main class="content">
      <div v-if="spacesLoading" class="state">Загрузка…</div>
      <div v-else-if="spacesError" class="state error">
        Ошибка загрузки spaces: {{ spacesError }}
      </div>
      <div v-else-if="spaces.length === 0" class="state">
        Ни одного space не найдено
      </div>
      <div v-else class="cards">
        <SpaceCard
          v-for="space in spaces"
          :key="space.id"
          :space="space"
          @open="openSpace"
        />
      </div>
    </main>

    <footer class="page-footer">
      <div>Все права защищены © 2026 YOSO Technologies Corp.</div>
      <div>Создано с любовью к данным, интернету и биологии человека.</div>
    </footer>
  </div>
</template>

<style scoped>
.welcome {
  width: 100%;
  height: 100vh;
  background: var(--background);
  color: var(--foreground);
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 64px 24px 32px;
  box-sizing: border-box;
  overflow: auto;
}

.title-stack {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.brand {
  font-size: 15px;
  font-weight: 500;
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
  letter-spacing: 0.2px;
}

.content {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  margin-top: 24px;
}

.cards {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  justify-content: center;
}

.state {
  text-align: center;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  font-size: 14px;
}

.state.error {
  color: var(--destructive);
}

.page-footer {
  margin-top: 32px;
  text-align: center;
  font-size: 11px;
  color: color-mix(in srgb, var(--foreground) 35%, transparent);
  line-height: 1.6;
}
</style>
