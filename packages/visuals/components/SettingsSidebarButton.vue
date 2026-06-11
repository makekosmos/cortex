<script setup lang="ts">
import type { Component } from "vue";

interface Props {
  icon: Component;
  label: string;
  active?: boolean;
  testId?: string;
  iconImage?: string;
  iconVariant?: "tile" | "plain";
  iconFrom?: string;
  iconTo?: string;
}

withDefaults(defineProps<Props>(), {
  active: false,
  testId: undefined,
  iconImage: "",
  iconVariant: "tile",
  iconFrom: "var(--settings-sidebar-icon-from)",
  iconTo: "var(--settings-sidebar-icon-to)",
});

defineEmits<{
  click: [];
  contextmenu: [event: MouseEvent];
}>();
</script>

<template>
  <button
    type="button"
    class="kosmos-settings-sidebar-button flex w-full cursor-default select-none items-center gap-2 rounded border-0 bg-transparent p-1 text-left text-[color-mix(in_srgb,var(--foreground)_78%,transparent)] data-[active=true]:bg-[var(--settings-sidebar-active)] data-[active=true]:text-[var(--foreground)]"
    :data-active="active ? 'true' : undefined"
    :data-testid="testId"
    :aria-current="active ? 'page' : undefined"
    @click="$emit('click')"
    @contextmenu="$emit('contextmenu', $event)"
  >
    <span
      class="inline-flex size-6 shrink-0 items-center justify-center overflow-hidden rounded text-white"
      :class="
        iconImage
          ? 'bg-none shadow-none'
          : iconVariant === 'plain'
            ? active
              ? 'bg-transparent shadow-none text-[var(--foreground)]'
              : 'bg-transparent shadow-none text-[color-mix(in_srgb,var(--foreground)_70%,transparent)]'
            : 'bg-linear-to-bl from-[var(--settings-sidebar-button-icon-from)] to-[var(--settings-sidebar-button-icon-to)] shadow-[inset_0_0_0_1px_color-mix(in_srgb,oklch(1_0_0)_6%,transparent)]'
      "
      :style="{
        '--settings-sidebar-button-icon-from': iconFrom,
        '--settings-sidebar-button-icon-to': iconTo,
      }"
      aria-hidden="true"
    >
      <img v-if="iconImage" class="block size-6 object-contain" :src="iconImage" alt="" />
      <component v-else :is="icon" :size="14" :stroke-width="2" />
    </span>
    <span
      class="kosmos-settings-sidebar-button__label min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap font-[var(--font-sans)] text-[13px] leading-[1.4] font-medium"
    >
      {{ label }}
    </span>
  </button>
</template>
