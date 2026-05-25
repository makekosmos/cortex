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
    class="kosmos-blocklist-card"
    :class="{
      'kosmos-blocklist-card--active': active,
      'kosmos-blocklist-card--empty': previewLines.length === 0,
    }"
    role="button"
    tabindex="0"
    @click="onClick"
    @keydown="onKey"
  >
    <div class="kosmos-blocklist-card__preview" aria-hidden="true">
      <slot>
        <div v-for="(domain, i) in previewLines" :key="i" class="kosmos-blocklist-card__domain">
          {{ domain }}
        </div>
      </slot>

      <span v-if="preset" class="kosmos-blocklist-card__preset-badge">preset</span>

      <span v-if="active" class="kosmos-blocklist-card__active-dot" aria-hidden="true" />
    </div>

    <div class="kosmos-blocklist-card__footer">
      <span v-if="icon" class="kosmos-blocklist-card__icon">{{ icon }}</span>
      <div class="kosmos-blocklist-card__titlebox">
        <div class="kosmos-blocklist-card__title">{{ name }}</div>
        <div class="kosmos-blocklist-card__subtitle">{{ subtitle }}</div>
      </div>
      <div class="kosmos-blocklist-card__actions">
        <slot name="actions" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.kosmos-blocklist-card {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 140px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--background);
  overflow: hidden;
  transition:
    transform 0.12s ease,
    border-color 0.12s ease,
    box-shadow 0.12s ease;
  user-select: none;
}

.kosmos-blocklist-card:hover {
  transform: scale(1.02);
  border-color: color-mix(in srgb, var(--foreground) 30%, var(--border));
}

.kosmos-blocklist-card:focus-visible {
  outline: none;
  border-color: var(--primary);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--primary) 35%, transparent);
}

.kosmos-blocklist-card--active {
  border: 2px solid var(--primary);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--primary) 25%, transparent);
}

.kosmos-blocklist-card__preview {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
  padding: 12px 12px 0;
  overflow: hidden;
  font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
  font-size: 11px;
  line-height: 1.4;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
  mask-image: linear-gradient(to bottom, black 0%, black 50%, transparent 100%);
  -webkit-mask-image: linear-gradient(to bottom, black 0%, black 50%, transparent 100%);
}

.kosmos-blocklist-card__domain {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.kosmos-blocklist-card--empty .kosmos-blocklist-card__preview::after {
  content: "—";
  position: absolute;
  top: 12px;
  left: 12px;
  color: color-mix(in srgb, var(--foreground) 40%, transparent);
}

.kosmos-blocklist-card__preset-badge {
  position: absolute;
  top: 8px;
  right: 8px;
  padding: 2px 6px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--secondary, var(--muted-foreground)) 70%, transparent);
  color: var(--secondary-foreground, var(--background));
  font-family: var(--font-sans, system-ui, sans-serif);
  font-size: 9px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: lowercase;
  pointer-events: none;
}

.kosmos-blocklist-card__active-dot {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--primary);
  box-shadow: 0 0 6px color-mix(in srgb, var(--primary) 80%, transparent);
}

.kosmos-blocklist-card__footer {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-top: 1px solid var(--border);
  background: color-mix(in srgb, var(--background) 92%, var(--foreground));
  user-select: none;
}

.kosmos-blocklist-card__icon {
  font-size: 18px;
  line-height: 1;
  flex-shrink: 0;
}

.kosmos-blocklist-card__titlebox {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1 1 auto;
}

.kosmos-blocklist-card__title {
  font-family: var(--font-sans, system-ui, sans-serif);
  font-size: 14px;
  font-weight: 600;
  color: var(--foreground);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.kosmos-blocklist-card__subtitle {
  font-family: var(--font-sans, system-ui, sans-serif);
  font-size: 11px;
  color: var(--muted-foreground);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.kosmos-blocklist-card__actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}
</style>
