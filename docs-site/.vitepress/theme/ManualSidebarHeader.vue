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
      { text: "Основы", link: "/manual/basics" },
      { text: "Платформы", link: "/manual/platforms" },
      { text: "Бета — FAQ", link: "/manual/beta/" },
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

function normalize(path: string) {
  return path.replace(/\/$/, "") || "/";
}

const currentPath = computed(() => normalize(route.path));

function isActive(link: string) {
  return currentPath.value === normalize(link);
}

const isManualHome = computed(
  () => currentPath.value === "/manual" || currentPath.value.startsWith("/manual") && !currentPath.value.startsWith("/manual/beta"),
);
const isBeta = computed(() => currentPath.value.startsWith("/manual/beta"));
</script>

<template>
  <div v-if="isManual" class="kosmos-manual-sidebar">
    <div class="section-nav">
      <a class="section-link" :class="{ active: isManualHome && !isBeta }" href="/manual/">
        <span class="icon" aria-hidden="true">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M8 3.75s-3-2-6.25-2v10.5c3.25 0 6.25 2 6.25 2m0-10.5s3-2 6.25-2v10.5c-3.25 0-6.25 2-6.25 2m0-10.5v10.5" />
          </svg>
        </span>
        Мануал
      </a>
      <a class="section-link" :class="{ active: isBeta }" href="/manual/beta/">
        <span class="icon" aria-hidden="true">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M8 4.75v2.836a1 1 0 0 0 .293.707l1.957 1.957m4-2.25a6.25 6.25 0 1 1-12.5 0 6.25 6.25 0 0 1 12.5 0Z" />
          </svg>
        </span>
        Бета мануал
      </a>
    </div>

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
      </ul>
    </nav>
  </div>
</template>

<style scoped>
.kosmos-manual-sidebar {
  display: flex;
  flex-direction: column;
}

/* Top section nav: Мануал / Бета мануал */
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

/* Nav sections */
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
</style>
