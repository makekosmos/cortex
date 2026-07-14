<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { formatReadableRussianDate } from "@/lib/objectFieldFormatting";
import { resolveObjectImageSrc } from "@/lib/objectImages";
import { SYSTEM_TYPE_BOOK_ID } from "@/lib/systemTypes";

const props = defineProps<{
  entry: Entry;
  noteType: NoteType | null;
  entriesById: Map<string, Entry>;
}>();

const emit = defineEmits<{
  openEntry: [entryId: string];
}>();

function parseHeaderProps(source: string): Record<string, unknown> {
  try {
    const parsed = JSON.parse(source || "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

const isBook = computed(() => props.entry.type_id === SYSTEM_TYPE_BOOK_ID);
const headerProps = computed(() => parseHeaderProps(props.entry.header_props_json));
const title = computed(() =>
  getEntryDisplayTitle(props.entry.title, props.entry.header_props_json),
);
const typeLabel = computed(() => props.noteType?.name ?? (isBook.value ? "Книга" : "Заметка"));
const author = computed(() => String(headerProps.value.author ?? "").trim());
const coverSrc = computed(() =>
  isBook.value ? resolveObjectImageSrc(headerProps.value.cover_image, props.entriesById) : "",
);
const coverCrossOrigin = computed(() =>
  /^kosmos-local-image:/i.test(coverSrc.value) ? "anonymous" : undefined,
);
const coverFailed = shallowRef(false);
const coverColor = shallowRef("");
const updatedLabel = computed(() => formatReadableRussianDate(props.entry.updated_at));
const updatedDateTime = computed(() => {
  const timestamp = Number(props.entry.updated_at);
  return Number.isFinite(timestamp) ? new Date(timestamp).toISOString() : undefined;
});
const openLabel = computed(() => `Открыть «${title.value}»`);

watch(coverSrc, () => {
  coverFailed.value = false;
  coverColor.value = "";
});

async function updateCoverColor(event: Event) {
  const image = event.currentTarget as HTMLImageElement;
  const source = coverSrc.value;
  const canvas = document.createElement("canvas");
  canvas.width = 32;
  canvas.height = 32;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) return;

  try {
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
    coverColor.value = dominant
      ? `rgb(${Math.round(dominant.red / dominant.count)} ${Math.round(dominant.green / dominant.count)} ${Math.round(dominant.blue / dominant.count)})`
      : "";
  } catch {
    const remoteColor = await window.kepler?.images?.dominantColor(source).catch(() => null);
    if (coverSrc.value === source) coverColor.value = remoteColor ?? "";
  }
}

function handleCoverError() {
  coverFailed.value = true;
  coverColor.value = "";
}
</script>

<template>
  <button
    type="button"
    class="everything-item-card"
    :class="isBook ? 'everything-item-card--book' : 'everything-item-card--note'"
    :aria-label="openLabel"
    :data-testid="`everything-card-${entry.id}`"
    @click="emit('openEntry', entry.id)"
  >
    <span class="everything-item-visual" :data-testid="`everything-visual-${entry.id}`">
      <template v-if="isBook">
        <span
          v-if="coverSrc && !coverFailed"
          class="everything-item-cover"
          :style="{ backgroundColor: coverColor || undefined }"
        >
          <img
            class="everything-item-cover-image"
            :src="coverSrc"
            :crossorigin="coverCrossOrigin"
            alt=""
            draggable="false"
            :data-testid="`everything-cover-${entry.id}`"
            @load="updateCoverColor"
            @error="handleCoverError"
          />
          <span
            class="everything-item-cover-layer"
            aria-hidden="true"
            :data-testid="`everything-cover-layer-${entry.id}`"
          />
        </span>
        <span
          v-else
          class="everything-item-cover-fallback"
          aria-hidden="true"
          :data-testid="`everything-cover-fallback-${entry.id}`"
        >
          Без обложки
        </span>
      </template>

      <span v-else class="everything-item-copy">
        <span class="everything-item-type">{{ typeLabel }}</span>
        <time class="everything-item-date" :datetime="updatedDateTime">{{ updatedLabel }}</time>
      </span>
    </span>

    <span class="everything-item-caption">
      <span class="everything-item-title">{{ title }}</span>
      <span v-if="isBook && author" class="everything-item-author">{{ author }}</span>
    </span>
  </button>
</template>

<style scoped>
.everything-item-card {
  display: flex;
  width: 100%;
  margin-block-end: var(--everything-gap);
  break-inside: avoid;
  flex-direction: column;
  gap: var(--space-1);
  padding: 0;
  appearance: none;
  border: 0;
  background: transparent;
  color: var(--foreground);
  text-align: left;
}

.everything-item-visual {
  display: block;
  width: 100%;
  overflow: hidden;
  border: 2px solid var(--card, transparent);
  border-radius: var(--radius-md);
  background: var(--card);
  transition:
    background-color var(--transition-normal),
    border-color var(--transition-normal);
}

.everything-item-card:hover .everything-item-visual {
  border-color: color-mix(in srgb, var(--foreground) 18%, var(--border));
  background: var(--surface);
}

.everything-item-card--book .everything-item-visual {
  padding: var(--space-4);
  border-color: transparent;
  background: transparent;
}

.everything-item-card:focus-visible {
  outline: none;
}

.everything-item-card:focus-visible .everything-item-visual {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.everything-item-cover {
  position: relative;
  display: block;
  width: 100%;
  overflow: hidden;
  border-radius: var(--radius-sm);
  background: var(--surface);
}

.everything-item-cover::before {
  position: absolute;
  z-index: 3;
  inset-block: 0;
  inset-inline-start: 0;
  width: 1.6%;
  background-color: inherit;
  content: "";
  pointer-events: none;
}

.everything-item-cover-image {
  display: block;
  width: 100%;
  height: auto;
}

.everything-item-cover-layer {
  position: absolute;
  z-index: 2;
  top: 0;
  left: 1.6%;
  width: calc(100% - 1.6%);
  height: 100%;
  pointer-events: none;
  background:
    linear-gradient(201deg, rgb(255 255 255 / 20%), transparent 47%) top / 100% 66.423% no-repeat,
    linear-gradient(90deg, rgb(255 255 255 / 30%), transparent 5.683%),
    linear-gradient(
      90deg,
      rgb(255 255 255 / 35%) 0%,
      transparent 1.6%,
      rgb(0 0 0 / 5%) 3.5%,
      rgb(0 0 0 / 15%) 5.7%,
      rgb(255 255 255 / 65%) 6%,
      transparent 10%
    ),
    linear-gradient(270deg, rgb(255 255 255 / 25%), transparent 2%);
}

.everything-item-cover-fallback {
  display: flex;
  aspect-ratio: 2 / 3;
  align-items: center;
  justify-content: center;
  padding: var(--space-5);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--foreground) 8%, var(--bg-app));
  color: var(--muted-foreground);
  font-size: var(--kosmos-text-body-size);
  text-align: center;
}

.everything-item-type,
.everything-item-date {
  color: var(--muted-foreground);
  font-size: var(--kosmos-text-caption-size);
  line-height: 1.4;
}

.everything-item-type {
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.everything-item-copy {
  display: flex;
  min-height: 9rem;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
}

.everything-item-caption {
  display: flex;
  min-width: 0;
  align-items: center;
  flex-direction: column;
  gap: var(--space-1);
  padding-inline: 2px;
  text-align: center;
}

.everything-item-title {
  display: block;
  width: 100%;
  overflow: hidden;
  color: color-mix(in srgb, var(--foreground) 40%, var(--bg-app));
  font-size: var(--kosmos-text-body-size);
  font-weight: 600;
  letter-spacing: var(--kosmos-text-subheading-letter-spacing);
  line-height: var(--kosmos-text-subheading-line-height);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.everything-item-author {
  overflow: hidden;
  color: var(--muted-foreground);
  font-size: var(--kosmos-text-body-size);
  line-height: 1.4;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.everything-item-copy .everything-item-date {
  margin-block-start: auto;
}
</style>
