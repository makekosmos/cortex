<script setup lang="ts">
import { ref } from "vue";
import Calendar from "../../components/Calendar.vue";

const pad = (n: number) => String(n).padStart(2, "0");
const todayIso = (() => {
  const d = new Date();
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
})();

const empty = ref<string | null>(null);
const picked = ref<string | null>(todayIso);
</script>

<template>
  <Story title="Calendar" group="datetime">
    <Variant title="Без выбора">
      <div class="story-canvas">
        <p class="story-label">Только сегодня подсвечен (`today`).</p>
        <div class="frame">
          <Calendar :value="empty" :today="todayIso" @pick="(iso) => (empty = iso)" />
        </div>
        <p class="muted">Picked: <code>{{ empty ?? "—" }}</code></p>
      </div>
    </Variant>

    <Variant title="С выбранной датой">
      <div class="story-canvas">
        <div class="frame">
          <Calendar :value="picked" :today="todayIso" @pick="(iso) => (picked = iso)" />
        </div>
        <p class="muted">Picked: <code>{{ picked }}</code></p>
      </div>
    </Variant>

    <Variant title="Зафиксированный today (для скриншотов)">
      <div class="story-canvas">
        <p class="story-label">`today=2026-05-16` — детерминированный кадр.</p>
        <div class="frame">
          <Calendar :value="'2026-05-16'" today="2026-05-16" />
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.frame {
  display: inline-block;
  background: var(--popover);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 0.75rem;
}
.muted {
  margin-top: 0.75rem;
  font-size: 0.8rem;
  color: var(--muted-foreground);
}
code {
  font-family: var(--font-mono);
}
</style>
