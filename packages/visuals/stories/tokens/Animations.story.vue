<script setup lang="ts">
import { ref } from "vue";
import { animations } from "../../tokens";

const playing = ref(false);

function play() {
  playing.value = false;
  // eslint-disable-next-line @typescript-eslint/no-unused-expressions
  void document.body.offsetHeight; // force reflow
  playing.value = true;
  setTimeout(() => (playing.value = false), 700);
}
</script>

<template>
  <Story title="Animations" group="tokens" :layout="{ type: 'single', iframe: false }">
    <Variant title="Easings & durations">
      <div class="story-canvas">
        <p class="story-label">Токены анимаций из `tokens/animations.ts` + CSS-vars `--easing-*`.</p>

        <table class="grid">
          <thead>
            <tr>
              <th>token</th>
              <th>value</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td><code>animations.fast</code></td>
              <td><code>{{ animations.fast }}</code></td>
            </tr>
            <tr>
              <td><code>animations.normal</code></td>
              <td><code>{{ animations.normal }}</code></td>
            </tr>
            <tr>
              <td><code>animations.slow</code></td>
              <td><code>{{ animations.slow }}</code></td>
            </tr>
            <tr>
              <td><code>animations.spring</code></td>
              <td><code>{{ animations.spring }}</code></td>
            </tr>
            <tr>
              <td><code>animations.sidebar.duration</code></td>
              <td><code>{{ animations.sidebar.duration }}</code></td>
            </tr>
            <tr>
              <td><code>--easing-standard</code></td>
              <td><code>cubic-bezier(0.4, 0, 0.2, 1)</code> — основные переходы</td>
            </tr>
            <tr>
              <td><code>--easing-emphasized</code></td>
              <td><code>cubic-bezier(0.2, 0, 0, 1)</code> — модалки, popover</td>
            </tr>
          </tbody>
        </table>

        <div class="demo">
          <button type="button" class="play" @click="play">Запустить демо</button>
          <div class="track">
            <div class="dot dot--fast" :class="{ run: playing }" />
            <code>fast</code>
          </div>
          <div class="track">
            <div class="dot dot--normal" :class="{ run: playing }" />
            <code>normal</code>
          </div>
          <div class="track">
            <div class="dot dot--slow" :class="{ run: playing }" />
            <code>slow</code>
          </div>
          <div class="track">
            <div class="dot dot--spring" :class="{ run: playing }" />
            <code>spring</code>
          </div>
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.grid {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.875rem;
  margin-bottom: 1.5rem;
}
.grid th,
.grid td {
  text-align: left;
  padding: 0.4rem 0.75rem;
  border-bottom: 1px solid var(--border);
}
.grid th {
  color: var(--muted-foreground);
  font-weight: 600;
  text-transform: uppercase;
  font-size: 0.7rem;
}
.demo {
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
  margin-top: 1rem;
}
.play {
  align-self: flex-start;
  padding: 0.45rem 0.85rem;
  border-radius: var(--radius-button);
  background: var(--accent);
  color: var(--accent-foreground);
  border: none;
  font-weight: 600;
  cursor: pointer;
}
.track {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  height: 24px;
  width: 360px;
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  border-radius: 999px;
  padding: 0 0.5rem;
}
.dot {
  width: 16px;
  height: 16px;
  border-radius: 999px;
  background: var(--accent);
  translate: 0 0;
}
.dot.run {
  animation-fill-mode: forwards;
}
.dot--fast.run {
  animation: slide 0.15s ease forwards;
}
.dot--normal.run {
  animation: slide 0.2s ease forwards;
}
.dot--slow.run {
  animation: slide 0.3s ease forwards;
}
.dot--spring.run {
  animation: slide 0.4s cubic-bezier(0.22, 1, 0.36, 1) forwards;
}
@keyframes slide {
  from {
    translate: 0 0;
  }
  to {
    translate: 280px 0;
  }
}
</style>
