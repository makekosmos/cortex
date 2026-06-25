<script setup lang="ts">
// AppCommandsTab — единый таб для четырёх «приложений» (notes/tasks/
// time-tracker/games).

import { Check } from "@lucide/vue";
import AdvancedPageLayout, { type IntroDescriptor } from "../components/AdvancedPageLayout.vue";
import LegacyRow from "../components/LegacyRow.vue";
import LegacyToggle from "../components/LegacyToggle.vue";
import type { AppSettingsTab } from "../navigation";

export interface AppCommandSetting {
  id: string;
  title: string;
  icon: string;
  iconFrom: string;
  iconTo: string;
  shortcut?: string;
}

defineProps<{
  intro: IntroDescriptor | null;
  activeTab: AppSettingsTab;
  commands: AppCommandSetting[];
  usageTracker: boolean;
  isCommandVisible: (id: string) => boolean;
}>();

defineEmits<{
  toggleUsageTracker: [e: Event];
  toggleCommandVisibility: [id: string, e: Event];
}>();
</script>

<template>
  <AdvancedPageLayout :intro="intro">
    <div v-if="activeTab === 'time-tracker'" class="rows command-settings-list time-settings-list">
      <LegacyRow
        title="Трекать активные приложения"
        hint="Записывает в ARK какое окно сейчас активно. Изменение применится после перезапуска Kepler."
      >
        <LegacyToggle
          :checked="usageTracker"
          @change="(e: Event) => $emit('toggleUsageTracker', e)"
        />
      </LegacyRow>
    </div>

    <div>
      <h2 class="advanced-section-title">Команды</h2>
      <div class="rows command-settings-list">
        <div v-for="command in commands" :key="command.id" class="row">
          <div class="command-row-label">
            <span
              class="command-row-icon"
              :style="{
                '--command-row-icon-from': command.iconFrom,
                '--command-row-icon-to': command.iconTo,
              }"
              aria-hidden="true"
            >
              <img :src="command.icon" alt="" />
            </span>
            <div class="command-row-text">
              <div class="label">{{ command.title }}</div>
              <code v-if="command.shortcut" class="command-row-shortcut">{{
                command.shortcut
              }}</code>
            </div>
          </div>
          <label class="command-checkbox" :aria-label="`Показывать ${command.title}`">
            <input
              type="checkbox"
              :checked="isCommandVisible(command.id)"
              @change="(e: Event) => $emit('toggleCommandVisibility', command.id, e)"
            />
            <span class="command-checkbox__box" aria-hidden="true">
              <Check :size="13" :stroke-width="3" />
            </span>
          </label>
        </div>
      </div>
    </div>
  </AdvancedPageLayout>
</template>

<style scoped>
.command-settings-list {
  width: 100%;
}

.time-settings-list {
  margin-top: 16px;
}

.command-row-label {
  display: inline-flex;
  min-width: 0;
  align-items: center;
  gap: 10px;
  flex: 1 1 auto;
}

.command-row-text {
  display: inline-flex;
  min-width: 0;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  flex: 1 1 auto;
}

.command-row-icon {
  display: inline-flex;
  width: 22px;
  height: 22px;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border-radius: 4px;
  background-image: linear-gradient(
    to bottom left,
    var(--command-row-icon-from),
    var(--command-row-icon-to)
  );
  box-shadow: inset 0 0 0 1px color-mix(in srgb, oklch(1 0 0) 6%, transparent);
  overflow: hidden;
}

.command-row-icon img {
  display: block;
  width: 16px;
  height: 16px;
  object-fit: contain;
}

.command-row-shortcut {
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
  padding: 2px 6px;
  border-radius: 4px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  color: color-mix(in srgb, var(--foreground) 78%, transparent);
  font-size: 0.6875rem;
  line-height: 1.2;
  letter-spacing: 0;
  white-space: nowrap;
}

.command-checkbox {
  position: relative;
  display: inline-flex;
  width: 20px;
  height: 20px;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.command-checkbox input {
  position: absolute;
  width: 0;
  height: 0;
  opacity: 0;
  pointer-events: none;
}

.command-checkbox__box {
  display: inline-flex;
  width: 16px;
  height: 16px;
  align-items: center;
  justify-content: center;
  border: 1px solid color-mix(in srgb, var(--foreground) 24%, transparent);
  border-radius: 4px;
  background: transparent;
  color: var(--main-background-color);
  transition:
    background 120ms ease,
    border-color 120ms ease,
    color 120ms ease;
}

.command-checkbox__box svg {
  opacity: 0;
  transform: scale(0.82);
  transition:
    opacity 120ms ease,
    transform 120ms ease;
}

.command-checkbox input:checked + .command-checkbox__box {
  border-color: var(--foreground);
  background: var(--foreground);
  color: #0d0d0d;
}

.command-checkbox input:checked + .command-checkbox__box svg {
  opacity: 1;
  transform: scale(1);
}

.command-checkbox input:focus-visible + .command-checkbox__box {
  outline: 2px solid color-mix(in srgb, var(--foreground) 35%, transparent);
  outline-offset: 2px;
}
</style>
