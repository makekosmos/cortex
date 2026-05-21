<script setup lang="ts">
import { SunMoon } from "lucide-vue-next";
import { useTheme } from "@/composables/useTheme";

const { theme, setTheme } = useTheme();
</script>

<template>
  <div class="settings-tab" data-testid="delphi-settings-general-tab">
    <header class="settings-tab-header">
      <p class="settings-tab-kicker">Настройки Delphi</p>
      <h1 class="settings-tab-title">Общие</h1>
      <p class="settings-tab-subtitle">Базовые настройки интерфейса и поведения приложения.</p>
    </header>

    <section class="settings-card">
      <div class="settings-card-header">
        <div class="settings-card-icon">
          <SunMoon :size="16" />
        </div>
        <div>
          <h2>Оформление</h2>
          <p>Выберите тему для окна Delphi.</p>
        </div>
      </div>

      <div class="settings-theme-grid">
        <button
          type="button"
          :class="[
            'settings-theme-button',
            theme === 'light' ? 'settings-theme-button--active' : '',
          ]"
          @click="setTheme('light')"
        >
          <span class="settings-theme-button-title">Светлая</span>
          <span class="settings-theme-button-desc">Яркая тема для дневной работы.</span>
        </button>

        <button
          type="button"
          :class="[
            'settings-theme-button',
            theme === 'dark' ? 'settings-theme-button--active' : '',
          ]"
          @click="setTheme('dark')"
        >
          <span class="settings-theme-button-title">Тёмная</span>
          <span class="settings-theme-button-desc">Более контрастный интерфейс для вечера.</span>
        </button>

        <button
          type="button"
          :class="[
            'settings-theme-button',
            theme === 'system' ? 'settings-theme-button--active' : '',
          ]"
          @click="setTheme('system')"
        >
          <span class="settings-theme-button-title">Системная</span>
          <span class="settings-theme-button-desc">Следовать системной теме устройства.</span>
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings-tab {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.settings-tab-header {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.settings-tab-kicker {
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--muted-foreground);
}

.settings-tab-title {
  font-size: clamp(1.8rem, 2.6vw, 2.4rem);
  line-height: 1;
  font-weight: 700;
  color: var(--foreground);
}

.settings-tab-subtitle {
  max-width: 42rem;
  font-size: 0.95rem;
  color: var(--muted-foreground);
}

.settings-card {
  border: 1px solid var(--border);
  border-radius: 1.25rem;
  background: color-mix(in srgb, var(--background) 86%, var(--secondary));
  overflow: hidden;
}

.settings-card-header {
  display: flex;
  align-items: center;
  gap: 0.9rem;
  padding: 1.2rem 1.25rem 1rem;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
}

.settings-card-header h2 {
  font-size: 1rem;
  font-weight: 600;
  color: var(--foreground);
}

.settings-card-header p {
  margin-top: 0.15rem;
  font-size: 0.85rem;
  color: var(--muted-foreground);
}

.settings-card-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 2rem;
  height: 2rem;
  border-radius: 0.75rem;
  background: color-mix(in srgb, var(--secondary) 82%, transparent);
  color: var(--foreground);
  flex-shrink: 0;
}

.settings-theme-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 0.85rem;
  padding: 1rem 1.25rem 1.25rem;
}

.settings-theme-button {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.35rem;
  padding: 1rem;
  border: 1px solid color-mix(in srgb, var(--border) 82%, transparent);
  border-radius: 1rem;
  background: color-mix(in srgb, var(--background) 92%, transparent);
  color: var(--foreground);
  text-align: left;
  transition:
    border-color 120ms ease,
    background-color 120ms ease,
    transform 120ms ease;
}

.settings-theme-button:hover {
  border-color: color-mix(in srgb, var(--foreground) 22%, var(--border));
  background: color-mix(in srgb, var(--secondary) 45%, var(--background));
  transform: translateY(-1px);
}

.settings-theme-button--active {
  border-color: color-mix(in srgb, var(--foreground) 26%, var(--border));
  background: color-mix(in srgb, var(--secondary) 80%, var(--background));
}

.settings-theme-button-title {
  font-size: 0.95rem;
  font-weight: 600;
}

.settings-theme-button-desc {
  font-size: 0.8rem;
  line-height: 1.4;
  color: var(--muted-foreground);
}
</style>
