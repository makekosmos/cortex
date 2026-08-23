<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vitepress";

const route = useRoute();
const isManual = computed(() => route.path.startsWith("/manual"));

const SECTIONS = [
  {
    title: "Начало",
    link: "/manual/basics",
    items: [
      { text: "Быстрый старт", link: "/manual/basics" },
      { text: "Платформы", link: "/manual/platforms" },
    ],
  },
  {
    title: "Core features",
    link: "/manual/core-features/",
    items: [
      { text: "Обзор", link: "/manual/core-features/" },
      { text: "Запуск приложений", link: "/manual/core-features/app-launcher" },
    ],
  },
  {
    title: "Встроенные расширения",
    link: "/manual/extensions/",
    items: [
      { text: "Обзор", link: "/manual/extensions/" },
      { text: "Eden — заметки", link: "/manual/extensions/eden" },
      { text: "Delphi — задачи", link: "/manual/extensions/delphi" },
      { text: "Horologion — время", link: "/manual/extensions/horologion" },
      { text: "Arrancador — игры", link: "/manual/extensions/arrancador" },
    ],
  },
];

const OTHER = [
  { text: "Новости", link: "/whats-new/" },
  { text: "Для разработчиков", link: "/guide/getting-started" },
];

function normalize(path: string) {
  return path.replace(/\/$/, "") || "/";
}

const currentPath = computed(() => normalize(route.path));

function isActive(link: string) {
  return currentPath.value === normalize(link);
}

const isBeta = computed(() => currentPath.value.startsWith("/manual/beta"));
// «Мануал» подсвечивается только на самой главной странице /manual/
const isManualHome = computed(() => currentPath.value === "/manual");

function openSearch() {
  if (typeof window === "undefined") return;
  // VitePress local search слушает Ctrl/Cmd+K глобально — эмулируем событие
  const event = new KeyboardEvent("keydown", {
    key: "k",
    code: "KeyK",
    ctrlKey: true,
    metaKey: true,
    bubbles: true,
    cancelable: true,
  });
  window.dispatchEvent(event);
  document.dispatchEvent(event);
  // Fallback — ищем стандартную кнопку поиска и кликаем
  const btn = document.querySelector<HTMLButtonElement>(
    ".VPNavBarSearch button, #docsearch button, .DocSearch-Button",
  );
  btn?.click();
}
</script>

<template>
  <div v-if="isManual" class="kosmos-manual-sidebar">
    <!-- Логотип Kosmos сверху сайдбара -->
    <a class="logo" href="/">Kosmos</a>

    <!-- Поисковик -->
    <button type="button" class="search-trigger" @click="openSearch">
      <span class="search-icon" aria-hidden="true">
        <svg
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="m10 10 4.25 4.25m-3-7.75a4.75 4.75 0 1 1-9.5 0 4.75 4.75 0 0 1 9.5 0Z" />
        </svg>
      </span>
      <span class="search-label">Искать</span>
      <span class="search-kbd-group">
        <kbd class="search-kbd">Ctrl</kbd>
        <kbd class="search-kbd">K</kbd>
      </span>
    </button>

    <!-- Section nav: Мануал / Бета мануал -->
    <div class="section-nav">
      <a class="section-link" :class="{ active: isManualHome }" href="/manual/">
        <span class="icon" aria-hidden="true">
          <svg
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path
              d="M8 3.75s-3-2-6.25-2v10.5c3.25 0 6.25 2 6.25 2m0-10.5s3-2 6.25-2v10.5c-3.25 0-6.25 2-6.25 2m0-10.5v10.5"
            />
          </svg>
        </span>
        Мануал
      </a>
      <a class="section-link" :class="{ active: isBeta }" href="/manual/beta/">
        <span class="icon" aria-hidden="true">
          <svg
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path
              d="M8 4.75v2.836a1 1 0 0 0 .293.707l1.957 1.957m4-2.25a6.25 6.25 0 1 1-12.5 0 6.25 6.25 0 0 1 12.5 0Z"
            />
          </svg>
        </span>
        Бета мануал
      </a>
    </div>

    <!-- Основная навигация -->
    <nav class="nav" aria-label="Manual navigation">
      <ul class="list">
        <li v-for="section in SECTIONS" :key="section.title" class="section">
          <div class="section-header">
            <a class="section-title" :href="section.link">{{ section.title }}</a>
          </div>
          <ul class="section-list">
            <li v-for="item in section.items" :key="item.link" class="item">
              <a class="link" :class="{ active: isActive(item.link) }" :href="item.link">
                {{ item.text }}
              </a>
            </li>
          </ul>
        </li>

        <!-- Другое -->
        <li class="section other-section">
          <div class="section-header">
            <span class="section-title as-label">Другое</span>
          </div>
          <ul class="section-list">
            <li v-for="o in OTHER" :key="o.link" class="item">
              <a class="link other-link" :href="o.link">
                <span>{{ o.text }}</span>
                <span class="arrow" aria-hidden="true">→</span>
              </a>
            </li>
          </ul>
        </li>
      </ul>
    </nav>
  </div>
</template>

<style scoped src="./ManualSidebarHeader.css"></style>
