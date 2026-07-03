<script setup lang="ts">
import { computed } from "vue";
import type { BookRecord } from "../lib/bookStorage";

const props = defineProps<{
  book: BookRecord;
  featured?: boolean;
  variant?: "compact" | "tile" | "featured";
}>();

const cardVariant = computed(() => props.variant ?? (props.featured ? "featured" : "tile"));
const progressPercent = computed(() => Math.round((props.book.progress?.percentage ?? 0) * 100));
const authors = computed(() => props.book.authors.join(", ") || "Автор не указан");
const coverInitials = computed(() =>
  props.book.title
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join(""),
);
</script>

<template>
  <button
    class="book-card"
    :class="{
      'book-card--compact': cardVariant === 'compact',
      'book-card--featured': cardVariant === 'featured',
      'book-card--tile': cardVariant === 'tile',
    }"
    type="button"
  >
    <span class="book-card__cover" aria-hidden="true">
      <img v-if="book.coverDataUrl" class="book-card__image" :src="book.coverDataUrl" alt="" />
      <span v-else class="book-card__cover-fallback">
        <span class="book-card__cover-initials">{{ coverInitials }}</span>
      </span>
    </span>
    <span class="book-card__body">
      <span class="book-card__title">{{ book.title }}</span>
      <span class="book-card__authors">{{ authors }}</span>
      <span class="book-card__progress" aria-hidden="true">
        <span class="book-card__progress-fill" :style="{ width: `${progressPercent}%` }" />
      </span>
      <span class="book-card__meta">
        {{ progressPercent > 0 ? `${progressPercent}% прочитано` : "Не начата" }}
      </span>
    </span>
  </button>
</template>

<style scoped>
.book-card {
  display: grid;
  width: 100%;
  min-width: 0;
  border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  border-radius: 8px;
  background: var(--card);
  color: var(--foreground);
  text-align: left;
  cursor: pointer;
  transition:
    border-color 140ms ease,
    background-color 140ms ease,
    transform 140ms ease;
}

.book-card:hover {
  border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
  background: color-mix(in srgb, var(--card) 92%, var(--muted));
  transform: translateY(-1px);
}

.book-card--compact {
  grid-template-columns: 54px minmax(0, 1fr);
  gap: 12px;
  min-width: 270px;
  max-width: 330px;
  min-height: 88px;
  padding: 12px;
}

.book-card--tile {
  grid-template-rows: auto minmax(0, 1fr);
  gap: 14px;
  min-height: 300px;
  padding: 18px;
}

.book-card--featured {
  grid-template-columns: 132px minmax(0, 1fr);
  gap: 16px;
  max-width: 600px;
  min-height: 214px;
  padding: 18px;
}

.book-card__cover {
  position: relative;
  display: block;
  overflow: hidden;
  border: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
  border-radius: 8px;
  background: var(--card);
  box-shadow: 0 18px 36px color-mix(in srgb, var(--foreground) 18%, transparent);
}

.book-card--compact .book-card__cover {
  aspect-ratio: 0.68;
  box-shadow: none;
}

.book-card--featured .book-card__cover {
  aspect-ratio: 0.68;
}

.book-card--tile .book-card__cover {
  width: min(100%, 150px);
  aspect-ratio: 0.68;
  justify-self: center;
}

.book-card__image,
.book-card__cover-fallback {
  width: 100%;
  height: 100%;
}

.book-card__image {
  display: block;
  object-fit: cover;
}

.book-card__cover-fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  background:
    linear-gradient(135deg, color-mix(in srgb, var(--accent) 22%, var(--card)), transparent 58%),
    linear-gradient(180deg, var(--card), color-mix(in srgb, var(--muted) 82%, var(--card)));
}

.book-card__cover-initials {
  color: var(--muted-foreground);
  font-family: "Source Serif 4", serif;
  font-size: 30px;
  font-weight: 700;
  letter-spacing: 0;
}

.book-card--compact .book-card__cover-initials {
  font-size: 16px;
}

.book-card__body {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.book-card--featured .book-card__body,
.book-card--compact .book-card__body {
  justify-content: center;
}

.book-card__title,
.book-card__authors,
.book-card__meta {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-card__title {
  font-size: 15px;
  font-weight: 700;
  letter-spacing: 0;
  line-height: 1.25;
}

.book-card--compact .book-card__title {
  font-size: 13px;
}

.book-card--featured .book-card__title {
  font-size: 20px;
}

.book-card__authors {
  margin-top: 4px;
  color: var(--muted-foreground);
  font-size: 12px;
}

.book-card--compact .book-card__authors {
  font-size: 11px;
}

.book-card__progress {
  position: relative;
  display: block;
  height: 4px;
  margin-top: 14px;
  overflow: hidden;
  border-radius: var(--radius-pill, 999px);
  background: var(--muted);
}

.book-card__progress-fill {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
}

.book-card__meta {
  margin-top: 8px;
  color: var(--muted-foreground);
  font-size: 12px;
}

.book-card:hover .book-card__title {
  color: var(--accent);
}
</style>
