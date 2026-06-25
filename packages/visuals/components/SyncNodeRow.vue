<script setup lang="ts">
import { computed } from "vue";
import { Laptop, Monitor, Smartphone } from "@lucide/vue";
import Button from "./Button.vue";
import StatusDot from "./StatusDot.vue";

type SyncNodeDeviceKind = "desktop" | "laptop" | "phone" | "unknown";
type SyncNodeStatus = "online" | "offline" | "connecting";

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
    default:
      return Monitor;
  }
});

const tone = computed(() =>
  props.status === "online" ? "success" : props.status === "connecting" ? "warning" : "danger",
);
</script>

<template>
  <div
    class="group flex items-center gap-3 rounded-lg border border-[var(--border)] bg-[var(--settings-list-background)] px-4 py-3"
  >
    <div
      class="flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--settings-list-background)] text-[var(--foreground)]"
    >
      <component :is="icon" :size="18" :stroke-width="2" />
    </div>

    <div class="min-w-0 flex-1">
      <div
        class="truncate font-[var(--font-sans)] text-[length:var(--kosmos-text-body-size)] leading-[1.4] font-medium text-[var(--foreground)]"
      >
        {{ name }}
      </div>
      <div
        class="mt-0.5 font-[var(--font-mono)] tabular-nums text-[length:var(--kosmos-text-caption-size)] leading-[1.4] text-[var(--muted-foreground)]"
      >
        {{ lastSeenLabel }}
      </div>
    </div>

    <div class="flex shrink-0 items-center gap-3">
      <Button
        size="sm"
        variant="danger"
        :loading="disconnecting"
        :disabled="disabled || disconnecting"
        class="opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
        @click="emit('disconnect')"
      >
        {{ disconnectLabel }}
      </Button>
      <StatusDot :tone="tone" :label="statusLabel ?? status" />
    </div>
  </div>
</template>
