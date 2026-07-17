<script setup lang="ts">
import Sidebar from "./Sidebar.vue";

interface Props {
  tone?: "default" | "strong";
  title?: string;
  background?: string;
}

withDefaults(defineProps<Props>(), {
  tone: "default",
  title: undefined,
  background: undefined,
});
</script>

<template>
  <Sidebar mode="panel" :tone="tone" :title="title" :background="background">
    <template v-if="$slots['title-leading']" #title-leading>
      <slot name="title-leading" />
    </template>
    <template v-if="$slots['title-trailing']" #title-trailing>
      <slot name="title-trailing" />
    </template>
    <slot />
  </Sidebar>
</template>
