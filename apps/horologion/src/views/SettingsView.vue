<script setup lang="ts">
import { Play } from "lucide-vue-next";
import { pomodoroSettings, resetPomodoroSettings } from "../lib/pomodoroSettings";
import { playSound, SOUND_OPTIONS, type SoundName } from "../lib/sounds";

function clampMin(v: number): number {
  return Math.max(1, Math.min(180, Math.floor(v)));
}
function clampCount(v: number): number {
  return Math.max(1, Math.min(20, Math.floor(v)));
}

function testSound(s: SoundName) {
  playSound(s);
}
</script>

<template>
  <div class="settings">
    <header class="settings__head">
      <h2 class="settings__title">Настройки помодоро</h2>
      <p class="settings__sub">Локально, в этом приложении.</p>
    </header>

    <section class="group">
      <h3 class="group__title">Длительности</h3>
      <label class="row">
        <span>Рабочее время</span>
        <span class="row__control">
          <input
            type="number"
            min="1"
            max="180"
            :value="pomodoroSettings.workMin"
            @change="(e) => (pomodoroSettings.workMin = clampMin(Number((e.target as HTMLInputElement).value)))"
          />
          <span class="row__unit">мин</span>
        </span>
      </label>
      <label class="row">
        <span>Короткий перерыв</span>
        <span class="row__control">
          <input
            type="number"
            min="1"
            max="60"
            :value="pomodoroSettings.shortBreakMin"
            @change="(e) => (pomodoroSettings.shortBreakMin = clampMin(Number((e.target as HTMLInputElement).value)))"
          />
          <span class="row__unit">мин</span>
        </span>
      </label>
      <label class="row">
        <span>Длинный перерыв</span>
        <span class="row__control">
          <input
            type="number"
            min="1"
            max="120"
            :value="pomodoroSettings.longBreakMin"
            @change="(e) => (pomodoroSettings.longBreakMin = clampMin(Number((e.target as HTMLInputElement).value)))"
          />
          <span class="row__unit">мин</span>
        </span>
      </label>
      <label class="row">
        <span>Помидорок до длинного перерыва</span>
        <span class="row__control">
          <input
            type="number"
            min="1"
            max="20"
            :value="pomodoroSettings.pomodorosUntilLongBreak"
            @change="(e) => (pomodoroSettings.pomodorosUntilLongBreak = clampCount(Number((e.target as HTMLInputElement).value)))"
          />
          <span class="row__unit">шт</span>
        </span>
      </label>
    </section>

    <section class="group">
      <h3 class="group__title">Поведение</h3>
      <label class="toggle">
        <input v-model="pomodoroSettings.trackBreaksAsRest" type="checkbox" />
        <span class="toggle__cap" />
        <span class="toggle__text">Трекать перерывы как «Отдых»</span>
      </label>
      <label class="toggle">
        <input v-model="pomodoroSettings.autoStartWork" type="checkbox" />
        <span class="toggle__cap" />
        <span class="toggle__text">Автостарт рабочего времени</span>
      </label>
      <label class="toggle">
        <input v-model="pomodoroSettings.autoStartBreak" type="checkbox" />
        <span class="toggle__cap" />
        <span class="toggle__text">Автостарт перерыва</span>
      </label>
      <label class="toggle">
        <input v-model="pomodoroSettings.systemNotifications" type="checkbox" />
        <span class="toggle__cap" />
        <span class="toggle__text">Системные уведомления</span>
      </label>
    </section>

    <section class="group">
      <h3 class="group__title">Звуки</h3>
      <label class="row">
        <span>Конец рабочего времени</span>
        <span class="row__control">
          <select v-model="pomodoroSettings.workEndSound" class="select">
            <option v-for="s in SOUND_OPTIONS" :key="s.value" :value="s.value">
              {{ s.label }}
            </option>
          </select>
          <button
            type="button"
            class="iconbtn"
            title="Прослушать"
            @click="testSound(pomodoroSettings.workEndSound)"
          >
            <Play :size="12" :stroke-width="2" />
          </button>
        </span>
      </label>
      <label class="row">
        <span>Конец перерыва</span>
        <span class="row__control">
          <select v-model="pomodoroSettings.breakEndSound" class="select">
            <option v-for="s in SOUND_OPTIONS" :key="s.value" :value="s.value">
              {{ s.label }}
            </option>
          </select>
          <button
            type="button"
            class="iconbtn"
            title="Прослушать"
            @click="testSound(pomodoroSettings.breakEndSound)"
          >
            <Play :size="12" :stroke-width="2" />
          </button>
        </span>
      </label>
    </section>

    <footer class="settings__foot">
      <button class="reset" @click="resetPomodoroSettings">Сбросить к дефолтам</button>
    </footer>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  max-width: 520px;
  margin: 0 auto;
}

.settings__head {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.settings__title {
  margin: 0;
  font-size: 1.0625rem;
  font-weight: 600;
}

.settings__sub {
  margin: 0;
  font-size: 0.8125rem;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
}

.group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.75rem 0.875rem;
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.85);
  corner-shape: var(--corner-shape);
  background: color-mix(in srgb, var(--foreground) 1.5%, var(--background));
}

.group__title {
  margin: 0 0 0.25rem 0;
  font-size: 0.6875rem;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  font-weight: 600;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  font-size: 0.875rem;
  color: var(--foreground);
}

.row__control {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}

.row__control input[type="number"] {
  width: 60px;
  height: 28px;
  text-align: right;
  padding: 0 0.4rem;
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.55);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  outline: none;
}

.row__control input[type="number"]:focus {
  border-color: color-mix(in srgb, var(--accent) 55%, transparent);
}

.row__control input[type="number"]::-webkit-inner-spin-button,
.row__control input[type="number"]::-webkit-outer-spin-button {
  -webkit-appearance: none;
  appearance: none;
  margin: 0;
}

.row__control input[type="number"] {
  -moz-appearance: textfield;
  appearance: textfield;
}

.row__unit {
  font-size: 0.75rem;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  min-width: 28px;
}

.select {
  height: 28px;
  padding: 0 0.5rem;
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.55);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: inherit;
  font-size: 0.8125rem;
  outline: none;
  cursor: pointer;
}

.select:focus {
  border-color: color-mix(in srgb, var(--accent) 55%, transparent);
}

.iconbtn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: 1px solid var(--border);
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  border-radius: 6px;
  cursor: pointer;
  transition: background-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.iconbtn:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.toggle {
  display: grid;
  grid-template-columns: 36px 1fr;
  align-items: start;
  gap: 0.625rem;
  cursor: pointer;
}

.toggle input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}

.toggle__cap {
  position: relative;
  width: 32px;
  height: 18px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--foreground) 18%, transparent);
  transition: background-color 160ms cubic-bezier(0.2, 0, 0, 1);
  margin-top: 2px;
}

.toggle__cap::after {
  content: "";
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 999px;
  background: var(--background);
  transition: transform 160ms cubic-bezier(0.2, 0, 0, 1);
}

.toggle input:checked ~ .toggle__cap {
  background: var(--accent);
}

.toggle input:checked ~ .toggle__cap::after {
  transform: translateX(14px);
}

.toggle__text {
  font-size: 0.875rem;
  align-self: center;
}

.settings__foot {
  display: flex;
  justify-content: flex-end;
}

.reset {
  background: transparent;
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.6);
  corner-shape: var(--corner-shape);
  padding: 0.4rem 0.875rem;
  color: color-mix(in srgb, var(--foreground) 75%, transparent);
  font-family: inherit;
  font-size: 0.8125rem;
  cursor: pointer;
  transition: background-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.reset:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}
</style>
