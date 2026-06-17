<script setup lang="ts">
import { computed } from "vue";
import { Cpu, Laptop, Monitor, Smartphone } from "@lucide/vue";
import Button from "./Button.vue";
import StatusDot from "./StatusDot.vue";

export type SyncNodeDeviceKind = "desktop" | "laptop" | "phone" | "unknown";
export type SyncNodeStatus = "online" | "offline" | "connecting";

const props = withDefaults(
  defineProps<{
    deviceKind: SyncNodeDeviceKind;
    name: string;
    lastSeenLabel: string;
    status: SyncNodeStatus;
    statusLabel?: string;
    disconnectLabel?: string;
    disabled?: boolean;
    disconnecting?: boolean;
  }>(),
  {
    statusLabel: undefined,
    disconnectLabel: "Отключить",
    disabled: false,
    disconnecting: false,
  },
);

const emit = defineEmits<{ disconnect: [] }>();

const icon = computed(() => {
  switch (props.deviceKind) {
    case "phone":
      return Smartphone;
    case "laptop":
      return Laptop;
    case "desktop":
      return Monitor;
    default:
      return Cpu;
  }
});

const tone = computed(() =>
  props.status === "online" ? "success" : props.status === "connecting" ? "warning" : "neutral",
);
</script>

<template>
  <div
    class="flex items-center gap-3 rounded-xl border border-[var(--border)] bg-[color-mix(in_srgb,var(--foreground)_3%,var(--background))] px-4 py-3"
  >
    <div
      class="flex size-10 items-center justify-center rounded-xl border border-[var(--border)] bg-[color-mix(in_srgb,var(--foreground)_4%,var(--background))] text-[var(--foreground)]"
    >
      <component :is="icon" :size="18" :stroke-width="2" />
    </div>

    <div class="min-w-0 flex-1">
      <div class="truncate text-[0.95rem] font-medium text-[var(--foreground)]">
        {{ name }}
      </div>
      <div class="mt-0.5 text-[0.8125rem] text-[var(--muted-foreground)]">
        {{ lastSeenLabel }}
      </div>
    </div>

    <div class="flex items-center gap-3">
      <StatusDot :tone="tone" :label="statusLabel ?? status" />
      <Button
        size="sm"
        variant="danger"
        :loading="disconnecting"
        :disabled="disabled || disconnecting"
        @click="emit('disconnect')"
      >
        {{ disconnectLabel }}
      </Button>
    </div>
  </div>
</template>
