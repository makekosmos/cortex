<script setup lang="ts">
import Skeleton from "@/components/Skeleton.vue";
</script>

<template>
  <ul class="space-y-2">
    <li
      v-for="i in 6"
      :key="i"
      class="flex w-full gap-5 border-2 border-(--background) px-7 py-3.5"
    >
      <Skeleton class="h-6 w-6 shrink-0 rounded-full bg-(--secondary)" />

      <div class="flex w-12 min-w-0 gap-2.5">
        <Skeleton class="h-full w-full bg-(--secondary)" />
      </div>

      <div class="flex w-full min-w-0 gap-2.5">
        <Skeleton class="h-full w-full bg-(--secondary)" />
      </div>
    </li>
  </ul>
</template>
