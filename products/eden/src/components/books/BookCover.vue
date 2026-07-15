<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";

const props = withDefaults(
  defineProps<{
    src?: string;
    alt?: string;
    imageTestId?: string;
    layerTestId?: string;
    fallbackTestId?: string;
  }>(),
  { src: "", alt: "" },
);

const failed = shallowRef(false);
const spineColor = shallowRef("");
let colorRequestId = 0;
const crossOrigin = computed(() =>
  /^kosmos-local-image:/i.test(props.src) ? "anonymous" : undefined,
);

watch(
  () => props.src,
  (source) => {
    failed.value = false;
    spineColor.value = "";
    colorRequestId += 1;
  },
  { immediate: true },
);

function dominantCanvasColor(image: HTMLImageElement): string {
  const canvas = document.createElement("canvas");
  canvas.width = 32;
  canvas.height = 32;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) return "";
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
  const colors = new Map<number, { count: number; red: number; green: number; blue: number }>();
  for (let index = 0; index < pixels.length; index += 4) {
    if (pixels[index + 3]! < 128) continue;
    const red = pixels[index]!;
    const green = pixels[index + 1]!;
    const blue = pixels[index + 2]!;
    const key = ((red >> 5) << 6) | ((green >> 5) << 3) | (blue >> 5);
    const color = colors.get(key);
    if (color) {
      color.count += 1;
      color.red += red;
      color.green += green;
      color.blue += blue;
    } else {
      colors.set(key, { count: 1, red, green, blue });
    }
  }

  let dominant: { count: number; red: number; green: number; blue: number } | undefined;
  let fallback = dominant;
  for (const color of colors.values()) {
    if (!fallback || color.count > fallback.count) fallback = color;
    const red = color.red / color.count;
    const green = color.green / color.count;
    const blue = color.blue / color.count;
    if (Math.max(red, green, blue) - Math.min(red, green, blue) < 24) continue;
    if (!dominant || color.count > dominant.count) dominant = color;
  }
  dominant ??= fallback;
  return dominant
    ? `rgb(${Math.round(dominant.red / dominant.count)} ${Math.round(dominant.green / dominant.count)} ${Math.round(dominant.blue / dominant.count)})`
    : "";
}

async function updateSpineColor(event: Event): Promise<void> {
  const source = props.src;
  const requestId = colorRequestId;
  try {
    const color = dominantCanvasColor(event.currentTarget as HTMLImageElement);
    if (requestId === colorRequestId && props.src === source) spineColor.value = color;
  } catch {
    const color = await window.kepler?.images?.dominantColor(source).catch(() => null);
    if (requestId === colorRequestId && props.src === source) spineColor.value = color ?? "";
  }
}

function handleError(): void {
  failed.value = true;
  spineColor.value = "";
}
</script>

<template>
  <span v-if="src && !failed" class="book-cover">
    <img
      class="book-cover__image"
      :src="src"
      :alt="alt"
      :crossorigin="crossOrigin"
      draggable="false"
      :data-testid="imageTestId"
      @load="updateSpineColor"
      @error="handleError"
    />
    <span
      class="book-cover__layer"
      aria-hidden="true"
      :data-testid="layerTestId"
      :style="{ '--book-cover-spine-color': spineColor || 'var(--surface)' }"
    />
  </span>
  <span v-else class="book-cover__fallback" :data-testid="fallbackTestId">
    <slot name="fallback">Без обложки</slot>
  </span>
</template>

<style scoped>
.book-cover {
  position: relative;
  display: block;
  width: 100%;
  overflow: hidden;
  border-radius: var(--radius-sm);
  background: var(--surface);
}

.book-cover__image {
  display: block;
  width: 100%;
  height: auto;
}

.book-cover__layer {
  /* Adapted from https://stackoverflow.com/a/57930124 by G-Cyrillus, CC BY-SA 4.0. */
  position: absolute;
  z-index: 1;
  inset: 0;
  pointer-events: none;
  background: linear-gradient(
    to right,
    var(--book-cover-spine-color) 3px,
    rgb(255 255 255 / 50%) 5px,
    rgb(255 255 255 / 25%) 7px,
    rgb(255 255 255 / 25%) 10px,
    transparent 12px,
    transparent 16px,
    rgb(255 255 255 / 25%) 17px,
    transparent 22px
  );
}

.book-cover__fallback {
  display: grid;
  width: 100%;
  aspect-ratio: 2 / 3;
  place-items: center;
  overflow: hidden;
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--foreground) 8%, var(--background));
  color: var(--muted-foreground);
  font-size: var(--kosmos-text-body-size);
  text-align: center;
}
</style>
