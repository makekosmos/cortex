<script lang="ts">
const notePreviewCache = new Map<string, { updatedAt: number; text: string }>();
const NOTE_PREVIEW_CHARACTER_LIMIT = 800;
const NOTE_PREVIEW_LOAD_CONCURRENCY = 4;
const NOTE_PREVIEW_LOAD_DEBOUNCE_MS = 100;
const notePreviewLoadQueue: Array<{
  cancelled: boolean;
  run: () => Promise<void>;
}> = [];
let activeNotePreviewLoads = 0;
let notePreviewLoadTimer: number | null = null;

function flushNotePreviewLoadQueue(): void {
  notePreviewLoadTimer = null;
  while (activeNotePreviewLoads < NOTE_PREVIEW_LOAD_CONCURRENCY) {
    const item = notePreviewLoadQueue.shift();
    if (!item) return;
    if (item.cancelled) continue;
    activeNotePreviewLoads += 1;
    void item.run().finally(() => {
      activeNotePreviewLoads -= 1;
      flushNotePreviewLoadQueue();
    });
  }
}

function scheduleNotePreviewLoad(run: () => Promise<void>): () => void {
  const item = { cancelled: false, run };
  notePreviewLoadQueue.push(item);
  if (notePreviewLoadTimer === null) {
    notePreviewLoadTimer = window.setTimeout(
      flushNotePreviewLoadQueue,
      NOTE_PREVIEW_LOAD_DEBOUNCE_MS,
    );
  }
  return () => {
    item.cancelled = true;
  };
}

function truncateNotePreview(text: string): string {
  const normalized = text.trim();
  if (normalized.length <= NOTE_PREVIEW_CHARACTER_LIMIT) return normalized;

  const slice = normalized.slice(0, NOTE_PREVIEW_CHARACTER_LIMIT);
  const wordBoundary = slice.lastIndexOf(" ");
  const end = wordBoundary >= NOTE_PREVIEW_CHARACTER_LIMIT * 0.75 ? wordBoundary : slice.length;
  return `${slice.slice(0, end).trimEnd()}…`;
}
</script>

<script setup lang="ts">
import {
  computed,
  onActivated,
  onBeforeUnmount,
  onDeactivated,
  onMounted,
  shallowRef,
  watch,
} from "vue";
import { bubblePlainText } from "@/components/bubbles/bubbleDiaryModel";
import BookCover from "@/components/books/BookCover.vue";
import { readEntryTiptapDoc } from "@/editor-content/content";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { resolveObjectImageSrc } from "@/lib/objectImages";
import { SYSTEM_TYPE_BOOK_ID } from "@/lib/systemTypes";

const props = defineProps<{
  entry: Entry;
  noteType: NoteType | null;
  entriesById: Map<string, Entry>;
}>();

const emit = defineEmits<{
  contextMenu: [event: MouseEvent, entryId: string];
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
const coverSrc = computed(() =>
  isBook.value ? resolveObjectImageSrc(headerProps.value.cover_image, props.entriesById) : "",
);
const openLabel = computed(() => `Открыть «${title.value}»`);
const cardElement = shallowRef<HTMLElement | null>(null);
const cachedPreview = notePreviewCache.get(props.entry.id);
const notePreview = shallowRef(
  cachedPreview?.updatedAt === props.entry.updated_at ? cachedPreview.text : "",
);
let previewObserver: IntersectionObserver | null = null;
let previewVisible = false;
let previewRequestKey = "";
let cancelScheduledPreview: (() => void) | null = null;

function loadNotePreview() {
  if (isBook.value) return;
  const id = props.entry.id;
  const updatedAt = props.entry.updated_at;
  const requestKey = `${id}:${updatedAt}`;
  const cached = notePreviewCache.get(id);
  if (cached?.updatedAt === updatedAt) {
    notePreview.value = cached.text;
    return;
  }
  if (previewRequestKey === requestKey) return;
  previewRequestKey = requestKey;
  cancelScheduledPreview?.();
  cancelScheduledPreview = scheduleNotePreviewLoad(async () => {
    cancelScheduledPreview = null;
    if (!previewVisible || props.entry.id !== id || props.entry.updated_at !== updatedAt) {
      if (previewRequestKey === requestKey) previewRequestKey = "";
      return;
    }

    try {
      const loaded =
        props.entry.content_loaded === false
          ? await window.api?.loadEntry(id, { contentOnly: true })
          : props.entry;
      if (!loaded || props.entry.id !== id || props.entry.updated_at !== updatedAt) return;
      const text = truncateNotePreview(bubblePlainText(readEntryTiptapDoc(loaded.content_json)));
      notePreviewCache.set(id, { updatedAt, text });
      notePreview.value = text;
    } catch {
      // The title remains usable when a preview cannot be loaded.
    } finally {
      if (previewRequestKey === requestKey) previewRequestKey = "";
    }
  });
}

function observePreview() {
  previewObserver?.disconnect();
  previewVisible = false;
  if (isBook.value || !cardElement.value) return;
  previewObserver = new IntersectionObserver(
    ([entry]) => {
      previewVisible = entry?.isIntersecting === true;
      if (previewVisible) void loadNotePreview();
    },
    {
      root: cardElement.value.closest(".everything-view"),
      rootMargin: "50% 0px",
    },
  );
  previewObserver.observe(cardElement.value);
}

onMounted(observePreview);
onActivated(observePreview);
onDeactivated(() => {
  previewVisible = false;
  cancelScheduledPreview?.();
  cancelScheduledPreview = null;
  previewRequestKey = "";
  previewObserver?.disconnect();
});
onBeforeUnmount(() => {
  cancelScheduledPreview?.();
  previewObserver?.disconnect();
});

watch(
  () => [props.entry.id, props.entry.updated_at, props.entry.content_loaded] as const,
  () => {
    const cached = notePreviewCache.get(props.entry.id);
    if (cached?.updatedAt === props.entry.updated_at) notePreview.value = cached.text;
    if (previewVisible) void loadNotePreview();
  },
);
</script>

<template>
  <button
    ref="cardElement"
    type="button"
    class="everything-item-card"
    :class="isBook ? 'everything-item-card--book' : 'everything-item-card--note'"
    :aria-label="openLabel"
    :data-testid="`everything-card-${entry.id}`"
    @click="emit('openEntry', entry.id)"
    @contextmenu="emit('contextMenu', $event, entry.id)"
  >
    <span class="everything-item-visual" :data-testid="`everything-visual-${entry.id}`">
      <template v-if="isBook">
        <BookCover
          :src="coverSrc"
          :image-test-id="`everything-cover-${entry.id}`"
          :layer-test-id="`everything-cover-layer-${entry.id}`"
          :fallback-test-id="`everything-cover-fallback-${entry.id}`"
        />
      </template>

      <span
        v-else
        class="everything-item-copy"
        :class="{ 'everything-item-copy--has-preview': notePreview }"
      >
        <span
          v-if="notePreview"
          class="everything-item-preview"
          :data-testid="`everything-preview-${entry.id}`"
        >
          {{ notePreview }}
        </span>
      </span>
    </span>

    <span class="everything-item-caption">
      <span class="everything-item-title">{{ title }}</span>
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
  background-color: var(--card);
  transition:
    background-color 300ms ease,
    border-color 300ms ease;
}

.everything-item-card:hover .everything-item-visual {
  border-color: color-mix(in srgb, var(--foreground) 18%, var(--border));
  background-color: var(--surface);
}

.everything-item-card--book .everything-item-visual {
  padding: var(--space-4);
  border-color: transparent;
  background-color: transparent;
}

.everything-item-card--note .everything-item-visual {
  border-radius: var(--radius-sm);
}

.everything-item-card:focus-visible {
  outline: none;
}

.everything-item-card:focus-visible .everything-item-visual {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.everything-item-copy {
  position: relative;
  display: flex;
  min-height: 9rem;
  padding: var(--space-4);
}

.everything-item-preview {
  display: block;
  width: 100%;
  height: 7.5em;
  overflow: hidden;
  color: color-mix(in srgb, var(--foreground) 72%, var(--bg-app));
  font-size: var(--kosmos-text-body-size);
  line-height: 1.5;
  mask-image: linear-gradient(to bottom, #000 calc(100% - 2.5rem), transparent 100%);
  white-space: pre-wrap;
  -webkit-mask-image: linear-gradient(to bottom, #000 calc(100% - 2.5rem), transparent 100%);
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
</style>
