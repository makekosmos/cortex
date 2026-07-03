<script setup lang="ts">
withDefaults(
  defineProps<{
    type?: "button" | "submit" | "reset";
    disabled?: boolean;
  }>(),
  {
    type: "button",
    disabled: false,
  },
);
</script>

<template>
  <button :type="type" :disabled="disabled" class="kosmos-titlebar-button">
    <slot />
  </button>
</template>
