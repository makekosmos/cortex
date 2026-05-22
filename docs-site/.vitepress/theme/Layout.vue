<script setup lang="ts">
import DefaultTheme from "vitepress/theme";
import { computed, watch, onMounted } from "vue";
import { useRoute } from "vitepress";
import ManualSidebarHeader from "./ManualSidebarHeader.vue";

const { Layout } = DefaultTheme;

const route = useRoute();

// user mode: /manual/* и /whats-new/* — упрощённый nav
// dev mode: всё остальное — полный nav
const mode = computed(() =>
  route.path.startsWith("/manual") || route.path.startsWith("/whats-new") ? "user" : "dev",
);

function applyMode(m: string) {
  if (typeof document === "undefined") return;
  document.body.dataset.kosmosMode = m;
}

onMounted(() => {
  applyMode(mode.value);
  watch(mode, applyMode);
});
</script>

<template>
  <Layout>
    <template #sidebar-nav-before>
      <ManualSidebarHeader />
    </template>
  </Layout>
</template>
