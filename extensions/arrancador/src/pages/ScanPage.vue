<script setup lang="ts">
// ScanPage — read-only вид уже-просканированных game_obj.
//
// Native scanner (folder walk + process detection) живёт в legacy
// `apps/arrancador/electron/main/services/`. Extension renderer не может
// spawn'ить scanner процесс. Phase 5: scanner либо остаётся в legacy
// Arrancador.exe, либо переезжает в kepler-backend extension.
//
// Здесь — listing того, что уже в ARK + объяснение, куда жмать "Scan".

import { EmptyState } from "@kepler/visuals";

import { useGames } from "../composables/useGames";

const { games, loading, error } = useGames();
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">Сканер</h1>
    <p class="arrancador-page__hint">
      Сканер запускается в legacy Arrancador.exe (Phase 5). Здесь
      отображаются игры, уже добавленные в ARK.
    </p>

    <div v-if="error" class="arrancador-error">{{ error }}</div>

    <EmptyState v-if="loading && games.length === 0" title="Загрузка…" />

    <EmptyState
      v-else-if="games.length === 0"
      title="Сканер не находил игр"
      description="Откройте Arrancador.exe → «Добавление игр» → «Выбрать папку», чтобы запустить сканер."
    />

    <div v-else class="arrancador-scan-list">
      <article
        v-for="game in games"
        :key="game.id"
        class="arrancador-scan-list__row"
      >
        <span class="arrancador-scan-list__name">{{ game.name }}</span>
        <span class="arrancador-scan-list__path">
          {{ game.exePath ?? "exe-путь не задан" }}
        </span>
      </article>
    </div>
  </section>
</template>
