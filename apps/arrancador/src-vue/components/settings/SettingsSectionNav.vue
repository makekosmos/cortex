<script setup lang="ts">
import { Database, HardDrive, Key, Monitor, Power, Shield, Sparkles } from "lucide-vue-next";
import type { SettingsSectionId, SettingsSectionItem } from "./types";

defineProps<{
  items: SettingsSectionItem[];
  activeSection: SettingsSectionId;
}>();

const emit = defineEmits<{
  select: [sectionId: SettingsSectionId];
}>();
</script>

<template>
  <div class="flex flex-wrap gap-2">
    <button
      v-for="item in items"
      :key="item.id"
      type="button"
      class="inline-flex items-center gap-2 rounded-full border px-3 py-2 text-sm transition-colors"
      :class="
        activeSection === item.id
          ? 'border-primary bg-primary text-primary-foreground shadow-sm'
          : 'border-border/70 bg-card/80 text-card-foreground hover:bg-accent/80'
      "
      :aria-pressed="activeSection === item.id"
      @click="emit('select', item.id)"
    >
      <Monitor
        v-if="item.icon === 'monitor'"
        class="h-4 w-4"
        :class="activeSection === item.id ? 'text-primary-foreground' : ''"
      />
      <Power
        v-else-if="item.icon === 'power'"
        class="h-4 w-4"
        :class="activeSection === item.id ? 'text-primary-foreground' : ''"
      />
      <Database
        v-else-if="item.icon === 'database'"
        class="h-4 w-4"
        :class="activeSection === item.id ? 'text-primary-foreground' : ''"
      />
      <Shield
        v-else-if="item.icon === 'shield'"
        class="h-4 w-4"
        :class="activeSection === item.id ? 'text-primary-foreground' : ''"
      />
      <HardDrive
        v-else-if="item.icon === 'hardDrive'"
        class="h-4 w-4"
        :class="activeSection === item.id ? 'text-primary-foreground' : ''"
      />
      <Sparkles
        v-else-if="item.icon === 'sparkles'"
        class="h-4 w-4"
        :class="activeSection === item.id ? 'text-primary-foreground' : ''"
      />
      <Key
        v-else
        class="h-4 w-4"
        :class="activeSection === item.id ? 'text-primary-foreground' : ''"
      />
      <span>{{ item.label }}</span>
    </button>
  </div>
</template>
