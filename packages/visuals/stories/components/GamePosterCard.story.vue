<script setup lang="ts">
import GamePosterCard from "../../components/GamePosterCard.vue";
import { Gamepad2 } from "lucide-vue-next";
</script>

<template>
  <Story title="GamePosterCard" group="primitives" :layout="{ type: 'single', iframe: true }">
    <Variant title="С обложкой">
      <div class="story-canvas">
        <p class="story-label">Карточка из Arrancador — стандартное соотношение 16:9.</p>
        <div class="grid">
          <GamePosterCard
            to="/games/balatro"
            title="Balatro"
            eyebrow="LocalThunk · 2024"
            cover-src="https://images.unsplash.com/photo-1611601679059-7d6e6ff3fac4?w=640&h=360&fit=crop"
          />
          <GamePosterCard
            to="/games/disco-elysium"
            title="Disco Elysium"
            eyebrow="ZA/UM · 2019"
            cover-src="https://images.unsplash.com/photo-1542751371-adc38448a05e?w=640&h=360&fit=crop"
          />
        </div>
      </div>
    </Variant>

    <Variant title="Без обложки (placeholder slot)">
      <div class="story-canvas">
        <div class="grid">
          <GamePosterCard to="/games/unknown" title="Unknown indie">
            <template #placeholder>
              <Gamepad2 :size="48" style="color:var(--muted-foreground);" />
            </template>
          </GamePosterCard>
          <GamePosterCard
            to="/games/no-eyebrow"
            title="Игра без подзаголовка с длинным-длинным названием, переносится"
          />
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 1rem;
  max-width: 720px;
}
</style>
