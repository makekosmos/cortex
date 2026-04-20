<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    id: string;
    label: string;
    description?: string;
    disabled?: boolean;
  }>(),
  {
    description: undefined,
    disabled: false,
  },
);

const model = defineModel<boolean>({ required: true });

function toggle() {
  if (props.disabled) {
    return;
  }

  model.value = !model.value;
}
</script>

<template>
  <div
    class="flex items-center justify-between gap-4 rounded-xl px-2 py-2 transition-colors"
    :class="disabled ? 'opacity-50' : 'hover:bg-accent/50'"
  >
    <button
      type="button"
      class="flex-1 text-left disabled:cursor-not-allowed"
      :disabled="disabled"
      @click="toggle"
    >
      <span :id="id" class="block text-sm font-medium">
        {{ label }}
      </span>
      <span v-if="description" class="mt-1 block text-xs text-muted-foreground">
        {{ description }}
      </span>
    </button>

    <button
      type="button"
      class="relative inline-flex h-6 w-11 shrink-0 items-center rounded-full border transition-colors disabled:cursor-not-allowed"
      :class="
        model
          ? 'border-primary bg-primary'
          : 'border-border/70 bg-background/70'
      "
      :disabled="disabled"
      role="switch"
      :aria-checked="model"
      :aria-labelledby="id"
      @click="toggle"
    >
      <span
        class="inline-block h-5 w-5 rounded-full bg-white shadow-sm transition-transform"
        :class="model ? 'translate-x-5' : 'translate-x-0.5'"
      />
    </button>
  </div>
</template>
