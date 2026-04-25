<script setup lang="ts">
import { ArrowLeft } from "lucide-vue-next";
import { RouterLink } from "vue-router";
</script>

<template>
  <RouterLink
    to="/"
    class="inline-flex items-center gap-2 text-sm text-muted-foreground transition-colors hover:text-foreground"
  >
    <ArrowLeft class="h-4 w-4" />
    Назад
  </RouterLink>
</template>
