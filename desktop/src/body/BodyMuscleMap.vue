<script setup lang="ts">
import { computed } from "vue";
import { muscleLabels, musclesForSide, type BodySide } from "./body-muscles";

const props = defineProps<{
  side: BodySide;
  values: Record<string, number>;
  valueLabels: Record<string, string>;
  selected?: string | null;
}>();

const emit = defineEmits<{ select: [muscle: string] }>();
const regions = computed(() => musclesForSide(props.side));
const silhouette = computed(() =>
  props.side === "front"
    ? [
        "42.4489796 2.85714286 40 11.8367347 42.0408163 19.5918367 46.122449 23.2653061 49.7959184 25.3061224 54.6938776 22.4489796 57.5510204 19.1836735 59.1836735 10.2040816 57.1428571 2.44897959 49.7959184 0",
        "33.877551 140 34.6938776 143.265306 35.5102041 147.346939 36.3265306 151.020408 35.1020408 156.734694 29.7959184 156.734694 27.3469388 152.653061 27.3469388 147.346939 30.2040816 144.081633",
        "65.7142857 140 72.244898 147.755102 72.244898 152.244898 69.7959184 157.142857 64.8979592 156.734694 62.8571429 151.020408",
      ]
    : [
        "50.6382979 0 45.9574468 0.85106383 40.8510638 5.53191489 40.4255319 12.7659574 45.106383 20 55.7446809 20 59.1489362 13.6170213 59.5744681 4.68085106 55.7446809 1.27659574",
        "34.4680851 153.191489 31.0638298 159.148936 33.6170213 166.382979 37.4468085 162.553191",
        "66.3829787 153.617021 62.9787234 162.978723 66.8085106 166.382979 69.3617021 159.148936",
        "28.5106383 195.744681 30.212766 195.744681 33.6170213 201.702128 30.6382979 220 28.5106383 213.617021 26.8085106 198.297872",
        "69.787234 195.744681 71.9148936 195.744681 73.6170213 198.297872 71.9148936 213.191489 70.212766 219.574468 67.2340426 202.12766",
      ],
);

function fillFor(muscle: string) {
  const intensity = Math.round(Math.max(0, Math.min(1, props.values[muscle] ?? 0)) * 100);
  return `color-mix(in srgb, var(--accent) ${intensity}%, var(--body-muscle-empty))`;
}

function accessibleLabel(muscle: string) {
  return `${muscleLabels[muscle] ?? muscle}: ${props.valueLabels[muscle] ?? "нет данных"}`;
}
</script>

<template>
  <figure class="muscle-map">
    <figcaption>{{ side === "front" ? "Спереди" : "Сзади" }}</figcaption>
    <svg
      viewBox="0 0 100 220"
      role="group"
      :aria-label="side === 'front' ? 'Мышцы спереди' : 'Мышцы сзади'"
    >
      <polygon
        v-for="points in silhouette"
        :key="points"
        :points="points"
        class="body-silhouette"
      />
      <template v-for="region in regions" :key="region.muscle">
        <polygon
          v-for="(points, index) in region.points"
          :key="`${region.muscle}-${index}`"
          :points="points"
          tabindex="0"
          role="img"
          :aria-label="accessibleLabel(region.muscle)"
          :class="['muscle-region', { 'muscle-region--selected': selected === region.muscle }]"
          :style="{ fill: fillFor(region.muscle) }"
          @click="emit('select', region.muscle)"
          @focus="emit('select', region.muscle)"
          @keydown.enter.prevent="emit('select', region.muscle)"
          @keydown.space.prevent="emit('select', region.muscle)"
        >
          <title>{{ accessibleLabel(region.muscle) }}</title>
        </polygon>
      </template>
    </svg>
  </figure>
</template>

<style scoped>
.muscle-map {
  display: flex;
  min-width: 150px;
  flex: 1;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  margin: 0;
}

.muscle-map figcaption {
  color: var(--muted-foreground);
  font-size: 0.6875rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.muscle-map svg {
  width: min(100%, 230px);
  height: min(56vh, 460px);
  overflow: visible;
}

.muscle-region {
  stroke: color-mix(in srgb, var(--background) 70%, transparent);
  stroke-width: 0.55;
  cursor: pointer;
  transition:
    fill 160ms ease,
    stroke 120ms ease,
    filter 120ms ease;
}

.body-silhouette {
  fill: var(--body-muscle-empty);
  stroke: color-mix(in srgb, var(--background) 70%, transparent);
  stroke-width: 0.55;
}

.muscle-region:hover,
.muscle-region:focus-visible,
.muscle-region--selected {
  stroke: var(--foreground);
  stroke-width: 1;
  filter: brightness(1.08);
  outline: none;
}
</style>
