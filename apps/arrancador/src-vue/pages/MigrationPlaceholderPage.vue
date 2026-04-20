<script setup lang="ts">
defineProps<{
  title: string;
  description: string;
}>();
</script>

<template>
  <div class="p-6">
    <div class="rounded-2xl border border-border/70 bg-card/70 p-6 shadow-[0_18px_40px_rgba(0,0,0,0.22)]">
      <h1 class="text-2xl font-semibold tracking-tight">{{ title }}</h1>
      <p class="mt-3 max-w-2xl text-sm leading-6 text-muted-foreground">
        {{ description }}
      </p>
    </div>
  </div>
</template>
