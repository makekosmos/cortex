<script setup lang="ts">
// 4 corner labels — fixed position, 20px padding от краёв экрана независимо
// от размера. На mobile (<= 720px) скрываем bottom labels, оставляем top.

const DOCS_URL = "https://docs-site-beige-phi.vercel.app/";
const ORG_URL = "https://github.com/yoso-industries";
const GITHUB_URL = "https://github.com/ksanrse";
</script>

<template>
  <!-- Top-left -->
  <div class="corner top-left">
    <a :href="DOCS_URL" target="_blank" rel="noopener">Документация</a>
    <span class="sep">,</span>
    <a :href="ORG_URL" target="_blank" rel="noopener">Yoso</a>
  </div>

  <!-- Top-right -->
  <div class="corner top-right">
    <span class="prefix">Найти меня —</span>
    <a :href="GITHUB_URL" target="_blank" rel="noopener">GitHub</a>
  </div>

  <!-- Bottom-left -->
  <div class="corner bottom-left hide-on-mobile">
    Local-first, agent native
  </div>

  <!-- Bottom-right -->
  <div class="corner bottom-right hide-on-mobile">
    Windows 1809
  </div>
</template>

<style scoped>
.corner {
  position: fixed;
  z-index: 10;
  font-size: 14px;
  font-weight: 500;
  color: var(--muted);
  user-select: none;
  pointer-events: auto;
}

.corner a {
  color: var(--muted);
  transition: color 120ms ease;
}
.corner a:hover {
  color: #095fff;
}

.sep {
  margin: 0 4px 0 2px;
}

.prefix {
  margin-right: 6px;
}

.top-left {
  top: var(--edge-pad);
  left: var(--edge-pad);
}

.top-right {
  top: var(--edge-pad);
  right: var(--edge-pad);
}

.bottom-left {
  bottom: var(--edge-pad);
  left: var(--edge-pad);
}

.bottom-right {
  bottom: var(--edge-pad);
  right: var(--edge-pad);
}

@media (max-width: 720px) {
  .corner {
    font-size: 13px;
  }
  .hide-on-mobile {
    display: none;
  }
}
</style>
