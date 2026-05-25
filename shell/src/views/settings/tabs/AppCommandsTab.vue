<script setup lang="ts">
// AppCommandsTab — единый таб для четырёх «приложений» (notes/tasks/
// time-tracker/games). Показывает usage-tracker toggle для time-tracker и
// список комманд из appCommandSettings соответствующей категории.

import { Check } from "@lucide/vue";
import { SettingsAdvancedIntro } from "@kosmos/visuals";
import LegacyToggle from "../components/LegacyToggle.vue";
import type { Component } from "vue";

export interface AppCommandSetting {
  id: string;
  title: string;
  icon: string;
  iconFrom: string;
  iconTo: string;
}

interface IntroDescriptor {
  icon: Component;
  label: string;
  description?: string;
  introImage?: string;
  iconGradient?: { from?: string; to?: string };
}

defineProps<{
  intro: IntroDescriptor | null;
  activeTab: "notes" | "tasks" | "time-tracker" | "games";
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
  <section class="advanced-page kosmos-scroll">
    <SettingsAdvancedIntro
      v-if="intro"
      :icon="intro.icon"
      :title="intro.label"
      :description="intro.description"
      :image-src="intro.introImage"
      :icon-from="intro.iconGradient?.from"
      :icon-to="intro.iconGradient?.to"
    />

    <div class="advanced-page__body">
      <div
        v-if="activeTab === 'time-tracker'"
        class="rows command-settings-list time-settings-list"
      >
        <div class="row">
          <div class="row-label">
            <div class="label">Трекать активные приложения</div>
            <div class="hint">
              Записывает в ARK какое окно сейчас активно. Изменение применится после перезапуска
              Kepler.
            </div>
          </div>
          <LegacyToggle
            :checked="usageTracker"
            @change="(e: Event) => $emit('toggleUsageTracker', e)"
          />
        </div>
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
              <div class="label">{{ command.title }}</div>
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
    </div>
  </section>
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

.command-checkbox {
  position: relative;
  display: inline-flex;
  width: 20px;
  height: 20px;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  cursor: pointer;
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
