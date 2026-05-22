<script setup lang="ts">
import DefaultTheme from "vitepress/theme";
import ManualSidebarHeader from "./ManualSidebarHeader.vue";

const { Layout } = DefaultTheme;
</script>

<template>
  <Layout>
    <template #sidebar-nav-before>
      <ManualSidebarHeader />
    </template>
  </Layout>
</template>
