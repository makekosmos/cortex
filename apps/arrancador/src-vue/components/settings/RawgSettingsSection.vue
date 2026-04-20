<script setup lang="ts">
import { ExternalLink, Key } from "lucide-vue-next";

const props = withDefaults(
  defineProps<{
    sectionId?: string;
    rawgApiKey: string;
  }>(),
  {
    sectionId: undefined,
  },
);

const emit = defineEmits<{
  updateRawgApiKey: [value: string];
}>();
</script>

<template>
  <section :id="props.sectionId" class="space-y-4">
    <div class="flex items-center gap-2">
      <Key class="h-5 w-5" />
      <h2 class="text-base font-semibold sm:text-lg">RAWG API</h2>
    </div>

    <div class="rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
      <label class="mb-2 block text-sm font-medium" for="rawg-api-key">
        API key (optional)
      </label>
      <input
        id="rawg-api-key"
        type="password"
        :value="props.rawgApiKey"
        class="flex h-11 w-full rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
        placeholder="Your RAWG API key"
        @input="emit('updateRawgApiKey', ($event.target as HTMLInputElement).value)"
      />
      <p class="mt-2 text-xs text-muted-foreground">
        Get a free key from
        <a
          href="https://rawg.io/apidocs"
          target="_blank"
          rel="noopener noreferrer"
          class="ml-1 inline-flex items-center gap-1 text-primary hover:underline"
        >
          RAWG.io
          <ExternalLink class="h-3 w-3" />
        </a>
        for richer metadata search results.
      </p>
    </div>
  </section>
</template>
