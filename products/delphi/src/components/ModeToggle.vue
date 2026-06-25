<script setup lang="ts">
import { Moon, Sun } from "@lucide/vue";
import { IconButton } from "@kosmos/visuals";
import { useTheme } from "@/composables/useTheme";

const { theme, toggleTheme } = useTheme();
</script>

<template>
  <IconButton
    :size="40"
    :radius="8"
    :title="theme === 'dark' ? 'Включить светлую тему' : 'Включить тёмную тему'"
    :aria-label="theme === 'dark' ? 'Включить светлую тему' : 'Включить тёмную тему'"
    @click="toggleTheme"
  >
    <Moon v-if="theme === 'dark'" :size="18" />
    <Sun v-else :size="18" />
  </IconButton>
</template>
