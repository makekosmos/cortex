<script setup lang="ts">
// CataloguePage — stub.
//
// Legacy `apps/arrancador/src-vue/pages/CataloguePage.vue` показывает RAWG
// browse + поиск + "Добавить в библиотеку". Эти write paths требуют
// `electronAPI.gamesApi.add` + `metadataApi.search/apply` — недоступны в
// extension. Phase 5 решит, где хост этих API (kepler-backend extension?).
import { EmptyState } from "@kepler/visuals";
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">Каталог</h1>
    <p class="arrancador-page__hint">
      Каталог RAWG и добавление игр пока доступны только в legacy
      Arrancador.exe. Здесь раздел появится после Phase 5 (write paths через
      kepler-backend extension).
    </p>
    <EmptyState
      title="Раздел недоступен"
      description="Откройте Arrancador.exe, чтобы найти и добавить новые игры. После синхронизации они появятся в Библиотеке здесь."
    />
  </section>
</template>
