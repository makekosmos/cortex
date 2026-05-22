<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vitepress";

const route = useRoute();
const isManual = computed(() => route.path.startsWith("/manual"));
</script>

<template>
  <div v-if="isManual" class="manual-header">
    <a class="manual-row" href="/manual/" aria-label="Beta Manual">
      <span class="icon" aria-hidden="true">
        <!-- Lucide clock -->
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10" />
          <polyline points="12 6 12 12 16 14" />
        </svg>
      </span>
      <span class="label">Beta Manual</span>
    </a>
    <a class="manual-row primary" href="/manual/" aria-label="Manual">
      <span class="icon" aria-hidden="true">
        <!-- Lucide book-open -->
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
          <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
        </svg>
      </span>
      <span class="label">Manual</span>
    </a>
  </div>
</template>

<style scoped>
.manual-header {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 14px 0 18px;
  margin-bottom: 6px;
  border-bottom: 1px solid var(--vp-c-divider);
}

.manual-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 10px;
  border-radius: 6px;
  color: var(--vp-c-text-2);
  font-size: 14px;
  font-weight: 500;
  text-decoration: none;
  transition: background 120ms ease, color 120ms ease;
}

.manual-row:hover {
  background: var(--vp-c-bg-soft);
  color: var(--vp-c-text-1);
}

.manual-row.primary {
  color: var(--vp-c-text-1);
  font-weight: 600;
}

.icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  flex-shrink: 0;
}

.icon svg {
  width: 16px;
  height: 16px;
}
</style>
