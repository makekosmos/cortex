<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vitepress";

const route = useRoute();
const isManual = computed(() => route.path.startsWith("/manual"));
</script>

<template>
  <div v-if="isManual" class="manual-header">
    <a class="manual-row" href="/manual/">
      <span class="icon" aria-hidden="true">
        <!-- Lucide book-open -->
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
          <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
        </svg>
      </span>
      <span class="label">Мануал</span>
    </a>
    <a class="manual-row" href="/manual/beta/">
      <span class="icon" aria-hidden="true">
        <!-- Lucide clock -->
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10" />
          <polyline points="12 6 12 12 16 14" />
        </svg>
      </span>
      <span class="label">Бета мануал</span>
    </a>
  </div>
</template>

<style scoped>
.manual-header {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 6px;
  margin-bottom: 14px;
}

.manual-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 0;
  font-size: 13.5px;
  letter-spacing: -0.1px;
  color: var(--kosmos-muted-fg);
  text-decoration: none;
  transition: color 120ms ease;
}

.manual-row:hover {
  color: var(--vp-c-brand-1);
}

.icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  opacity: 0.85;
}

.icon svg {
  width: 14px;
  height: 14px;
}
</style>
