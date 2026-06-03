<script setup lang="ts">
// BlocklistCard — карточка блок-листа в Settings → Focus.
// Сверху preview содержимого (домены) с gradient fade, снизу footer
// с иконкой и заголовком + опциональным subtitle (количество доменов).
//
// Slot `default` → override preview (например custom markdown с @mentions).
// Slot `actions` → дополнительные кнопки в footer рядом с title.

import { computed } from "vue";

interface Props {
  name: string;
  domains: string[];
  icon?: string;
  preset?: boolean;
  active?: boolean;
  count?: number;
}

const props = withDefaults(defineProps<Props>(), {
  icon: "",
  preset: false,
  active: false,
  count: undefined,
});

const emit = defineEmits<{
  click: [];
  delete: [];
}>();

const effectiveCount = computed(() =>
  typeof props.count === "number" ? props.count : props.domains.length,
);

const subtitle = computed(() => {
  const n = effectiveCount.value;
  if (n === 0) return "пусто";
  // Russian pluralization для "домен / домена / доменов"
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return `${n} домен`;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return `${n} домена`;
  return `${n} доменов`;
});

const previewLines = computed(() => props.domains.slice(0, 12));

function onClick() {
  emit("click");
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    emit("click");
  }
}
</script>

<template>
  <div
    class="relative flex h-36 w-full select-none flex-col overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--background)] transition-[transform,border-color,box-shadow] duration-150 ease-out hover:scale-[1.02] hover:border-[color-mix(in_srgb,var(--foreground)_30%,var(--border))] focus-visible:border-[var(--primary)] focus-visible:shadow-[0_0_0_2px_color-mix(in_srgb,var(--primary)_35%,transparent)] focus-visible:outline-none"
    :class="{
      'border-2 border-[var(--primary)] shadow-[0_0_0_1px_color-mix(in_srgb,var(--primary)_25%,transparent)]':
        active,
    }"
    role="button"
    tabindex="0"
    @click="onClick"
    @keydown="onKey"
  >
    <div
      class="relative min-h-0 flex-auto overflow-hidden px-4 pt-4 font-[var(--font-mono,ui-monospace,SFMono-Regular,Menlo,monospace)] text-[11px] leading-[1.4] text-[color-mix(in_srgb,var(--foreground)_60%,transparent)] [mask-image:linear-gradient(to_bottom,black_0%,black_50%,transparent_100%)] [-webkit-mask-image:linear-gradient(to_bottom,black_0%,black_50%,transparent_100%)]"
      aria-hidden="true"
    >
      <slot>
        <div
          v-for="(domain, i) in previewLines"
          :key="i"
          class="overflow-hidden text-ellipsis whitespace-nowrap"
        >
          {{ domain }}
        </div>
      </slot>

      <span
        v-if="previewLines.length === 0"
        class="absolute top-4 left-4 text-[color-mix(in_srgb,var(--foreground)_40%,transparent)]"
      >
        —
      </span>

      <span
        v-if="preset"
        class="pointer-events-none absolute top-2 right-2 h-4 rounded-full bg-[color-mix(in_srgb,var(--secondary,var(--muted-foreground))_70%,transparent)] px-2 font-[var(--font-sans,system-ui,sans-serif)] text-[9px] font-semibold leading-4 tracking-[0.04em] text-[var(--secondary-foreground,var(--background))] lowercase"
      >
        preset
      </span>

      <span
        v-if="active"
        class="absolute top-2 right-2 size-2 rounded-full bg-[var(--primary)] shadow-[0_0_6px_color-mix(in_srgb,var(--primary)_80%,transparent)]"
        aria-hidden="true"
      />
    </div>

    <div
      class="flex select-none items-center gap-2 border-t border-[var(--border)] bg-[color-mix(in_srgb,var(--background)_92%,var(--foreground))] px-4 py-2"
    >
      <span v-if="icon" class="shrink-0 text-lg leading-none">{{ icon }}</span>
      <div class="flex min-w-0 flex-auto flex-col">
        <div
          class="overflow-hidden text-ellipsis whitespace-nowrap font-[var(--font-sans,system-ui,sans-serif)] text-sm font-semibold text-[var(--foreground)]"
        >
          {{ name }}
        </div>
        <div
          class="overflow-hidden text-ellipsis whitespace-nowrap font-[var(--font-sans,system-ui,sans-serif)] text-[11px] text-[var(--muted-foreground)]"
        >
          {{ subtitle }}
        </div>
      </div>
      <div class="flex shrink-0 items-center gap-2">
        <slot name="actions" />
      </div>
    </div>
  </div>
</template>
