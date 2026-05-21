<script setup lang="ts">
import { Moon, Sun } from "@lucide/vue";
import { useTheme } from "@/composables/useTheme";

const { theme, toggleTheme } = useTheme();
</script>

<template>
  <div class="inline-flex items-center gap-2 rounded-xl p-1 hover:bg-(--secondary)">
    <button type="button" class="rounded-md p-3 text-sm" @click="toggleTheme">
      <Moon v-if="theme === 'dark'" />
      <Sun v-else />
    </button>
  </div>
</template>
