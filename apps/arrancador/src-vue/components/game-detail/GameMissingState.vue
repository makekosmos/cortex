<script setup lang="ts">
import { RouterLink } from "vue-router";
</script>

<template>
  <div class="flex h-full items-center justify-center p-6">
    <div class="text-center">
      <div class="text-lg font-semibold">Игра не найдена</div>
      <RouterLink to="/" class="mt-3 inline-flex text-primary hover:underline">
        Вернуться в библиотеку
      </RouterLink>
    </div>
  </div>
</template>
