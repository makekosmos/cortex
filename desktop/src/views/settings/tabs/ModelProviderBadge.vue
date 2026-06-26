<script setup lang="ts">
import { PhMicrophone, PhTextT } from "@phosphor-icons/vue";
import ProviderIcon from "./ProviderIcon.vue";

defineProps<{
  kind: "groq" | "openai" | "nvidia" | "speech" | "text";
}>();
</script>

<template>
  <span class="model-badge" :class="`model-badge--${kind}`">
    <ProviderIcon v-if="kind === 'groq'" provider="groq" :size="14" />
    <ProviderIcon v-else-if="kind === 'openai'" provider="openai" :size="11" />
    <ProviderIcon v-else-if="kind === 'nvidia'" provider="nvidia" :size="15" />
    <PhMicrophone v-else-if="kind === 'speech'" :size="14" weight="bold" aria-hidden="true" />
    <PhTextT v-else :size="14" weight="bold" aria-hidden="true" />
  </span>
</template>

<style scoped>
.model-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  flex: 0 0 18px;
  border-radius: 5px;
  background: color-mix(in srgb, currentColor 7%, transparent);
  color: color-mix(in srgb, currentColor 88%, transparent);
}

.model-badge--groq {
  background: color-mix(in srgb, currentColor 9%, transparent);
}

.model-badge--openai {
  background: #fff;
  color: #000;
}

.model-badge--nvidia {
  background: #76b900;
  color: #111;
}

.model-badge--nvidia :deep(.provider-icon) {
  transform: translateX(-0.4px) translateY(0.2px);
}
</style>
