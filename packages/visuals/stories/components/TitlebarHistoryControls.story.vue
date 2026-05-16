<script setup lang="ts">
import { ref } from "vue";
import TitlebarHistoryControls from "../../components/TitlebarHistoryControls.vue";

const log = ref<string[]>([]);

function record(msg: string) {
  log.value = [msg, ...log.value].slice(0, 6);
}
</script>

<template>
  <Story title="TitlebarHistoryControls" group="primitives">
    <Variant title="Активные">
      <div class="story-canvas">
        <p class="story-label">Эмиссии events записываются в журнал ниже.</p>
        <div class="story-frame" style="background:var(--sidebar-bg);">
          <TitlebarHistoryControls
            @back="record('back')"
            @forward="record('forward')"
          />
        </div>
        <ul class="log">
          <li v-for="(m, i) in log" :key="i">{{ m }}</li>
        </ul>
      </div>
    </Variant>

    <Variant title="Назад отключён">
      <div class="story-canvas">
        <div class="story-frame" style="background:var(--sidebar-bg);">
          <TitlebarHistoryControls :back-disabled="true" />
        </div>
      </div>
    </Variant>

    <Variant title="Оба отключены">
      <div class="story-canvas">
        <div class="story-frame" style="background:var(--sidebar-bg);">
          <TitlebarHistoryControls
            :back-disabled="true"
            :forward-disabled="true"
          />
        </div>
      </div>
    </Variant>

    <Variant title="Кастомные tooltip’ы">
      <div class="story-canvas">
        <div class="story-frame" style="background:var(--sidebar-bg);">
          <TitlebarHistoryControls
            back-title="Предыдущая заметка"
            forward-title="Следующая заметка"
          />
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.log {
  margin: 1rem 0 0;
  padding: 0;
  list-style: none;
  font-family: var(--font-mono);
  font-size: 0.75rem;
  color: var(--muted-foreground);
}
.log li {
  padding: 0.15rem 0;
}
</style>
