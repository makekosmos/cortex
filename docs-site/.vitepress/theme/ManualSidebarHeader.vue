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

<style scoped>
.kosmos-manual-sidebar {
  display: flex;
  flex-direction: column;
}

/* Логотип */
.logo {
  display: block;
  padding: 4px 0 16px;
  font-size: 1.05rem;
  font-weight: 700;
  letter-spacing: -0.3px;
  color: var(--kosmos-fg);
  text-decoration: none;
}

/* Search trigger */
.search-trigger {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 12px;
  margin-bottom: 18px;
  background: var(--kosmos-sidebar-surface);
  border: 1px solid var(--kosmos-border);
  border-radius: 8px;
  color: var(--kosmos-muted-fg);
  font-size: 0.875rem;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition:
    border-color 120ms ease,
    background 120ms ease;
}

.search-trigger:hover {
  border-color: var(--kosmos-muted-fg);
}

.search-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  flex-shrink: 0;
}

.search-icon svg {
  width: 14px;
  height: 14px;
}

.search-label {
  flex: 1;
}

.search-kbd-group {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.search-kbd {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  font-family: var(--kosmos-font-mono);
  font-size: 11px;
  color: var(--kosmos-muted-fg);
  background: var(--vp-c-bg);
  border: 1px solid var(--kosmos-border);
  border-radius: 4px;
}

/* Section nav: Мануал / Бета мануал */
.section-nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-bottom: 14px;
}

.section-link {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 0;
  font-size: 0.875rem;
  color: var(--kosmos-muted-fg);
  text-decoration: none;
  transition: color 120ms ease;
}

.section-link:hover {
  color: var(--vp-c-brand-1);
}

.section-link.active {
  color: var(--vp-c-brand-1);
  font-weight: 600;
}

.section-link .icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  flex-shrink: 0;
}

.section-link .icon svg {
  width: 14px;
  height: 14px;
}

/* Sections */
.nav {
  display: block;
}

.list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.section {
  padding-bottom: 8px;
  list-style: none;
}

.section-header {
  padding: 4px 0;
  margin-bottom: 4px;
}

.section-title {
  font-size: 0.875rem;
  font-weight: 600;
  color: var(--kosmos-fg);
  text-decoration: none;
  letter-spacing: -0.1px;
  transition: color 120ms ease;
}

.section-title:hover {
  color: var(--vp-c-brand-1);
}

.section-title.as-label {
  cursor: default;
}

.section-title.as-label:hover {
  color: var(--kosmos-fg);
}

.section-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.item {
  list-style: none;
}

.link {
  display: block;
  padding: 5px 0;
  font-size: 0.875rem;
  font-weight: 400;
  color: var(--kosmos-muted-fg);
  text-decoration: none;
  letter-spacing: -0.1px;
  transition: color 120ms ease;
}

.link:hover {
  color: var(--vp-c-brand-1);
}

.link.active {
  color: var(--vp-c-brand-1);
  font-weight: 500;
}

/* "Другое" — items со стрелкой */
.other-section {
  margin-top: 14px;
}

.other-link {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.other-link .arrow {
  color: var(--kosmos-muted-fg);
  transition:
    transform 120ms ease,
    color 120ms ease;
}

.other-link:hover .arrow {
  color: var(--vp-c-brand-1);
  transform: translateX(2px);
}
</style>
