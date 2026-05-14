<script setup lang="ts">
import { computed } from "vue";
import { Play } from "lucide-vue-next";
import { Dropdown } from "@kosmos/visuals";
import { pomodoroSettings, resetPomodoroSettings } from "../lib/pomodoroSettings";
import { playSound, SOUND_OPTIONS, type SoundName } from "../lib/sounds";

const soundOptions = computed(() =>
    SOUND_OPTIONS.map((s) => ({ value: s.value, label: s.label })),
);

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
        <section class="group">
            <h3 class="group__title">Длительности</h3>
            <label class="row">
                <span>Рабочее время</span>
                <span class="row__control">
                    <input type="number" min="1" max="180" :value="pomodoroSettings.workMin"
                        @change="(e) => (pomodoroSettings.workMin = clampMin(Number((e.target as HTMLInputElement).value)))" />
                    <span class="row__unit">мин</span>
                </span>
            </label>
            <label class="row">
                <span>Короткий перерыв</span>
                <span class="row__control">
                    <input type="number" min="1" max="60" :value="pomodoroSettings.shortBreakMin"
                        @change="(e) => (pomodoroSettings.shortBreakMin = clampMin(Number((e.target as HTMLInputElement).value)))" />
                    <span class="row__unit">мин</span>
                </span>
            </label>
            <label class="row">
                <span>Длинный перерыв</span>
                <span class="row__control">
                    <input type="number" min="1" max="120" :value="pomodoroSettings.longBreakMin"
                        @change="(e) => (pomodoroSettings.longBreakMin = clampMin(Number((e.target as HTMLInputElement).value)))" />
                    <span class="row__unit">мин</span>
                </span>
            </label>
            <label class="row">
                <span>Помидорок до длинного перерыва</span>
                <span class="row__control">
                    <input type="number" min="1" max="20" :value="pomodoroSettings.pomodorosUntilLongBreak"
                        @change="(e) => (pomodoroSettings.pomodorosUntilLongBreak = clampCount(Number((e.target as HTMLInputElement).value)))" />
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
            <label class="toggle">
                <input v-model="pomodoroSettings.streamerMode" type="checkbox" />
                <span class="toggle__cap" />
                <span class="toggle__text">
                    Режим стримера
                    <span class="toggle__hint">Не приостанавливать отрисовку, когда окно перекрыто другим. Приложение перезапустится.</span>
                </span>
            </label>
        </section>

        <section class="group">
            <h3 class="group__title">Звуки</h3>
            <label class="row">
                <span>Конец рабочего времени</span>
                <span class="row__control">
                    <Dropdown
                        v-model="pomodoroSettings.workEndSound"
                        :options="soundOptions"
                        class="dd"
                    />
                    <button type="button" class="iconbtn" title="Прослушать"
                        @click="testSound(pomodoroSettings.workEndSound)">
                        <Play :size="12" :stroke-width="2" />
                    </button>
                </span>
            </label>
            <label class="row">
                <span>Конец перерыва</span>
                <span class="row__control">
                    <Dropdown
                        v-model="pomodoroSettings.breakEndSound"
                        :options="soundOptions"
                        class="dd"
                    />
                    <button type="button" class="iconbtn" title="Прослушать"
                        @click="testSound(pomodoroSettings.breakEndSound)">
                        <Play :size="12" :stroke-width="2" />
                    </button>
                </span>
            </label>
            <label class="row">
                <span>Громкость</span>
                <span class="row__control row__control--volume">
                    <input
                        type="range"
                        min="0"
                        max="100"
                        step="1"
                        class="volume-slider"
                        :value="Math.round(pomodoroSettings.ringtoneVolume * 100)"
                        @input="(e) => (pomodoroSettings.ringtoneVolume = Number((e.target as HTMLInputElement).value) / 100)"
                    />
                    <span class="row__unit">{{ Math.round(pomodoroSettings.ringtoneVolume * 100) }}%</span>
                    <button type="button" class="iconbtn" title="Прослушать"
                        @click="testSound(pomodoroSettings.workEndSound)">
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
    gap: 1rem;
    max-width: 520px;
    margin: 0 auto;
    width: 100%;
}

/* Группа = card-обёртка как .home__pomo / .day. */
.group {
    display: flex;
    flex-direction: column;
    padding: 1rem 1.25rem 1.25rem;
    border: 1px solid var(--border);
    border-radius: calc(var(--radius) * 3);
    corner-shape: var(--corner-shape);
    background: color-mix(in srgb, var(--foreground) 4%, var(--background));
}

.group__title {
    margin: 0 0 0.875rem 0;
    font-size: 0.6875rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: color-mix(in srgb, var(--foreground) 55%, transparent);
    font-weight: 600;
}

.row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.5rem 0;
    font-size: 0.9375rem;
    color: var(--foreground);
    border-top: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
}

.row:first-of-type {
    border-top: none;
    padding-top: 0;
}

.row__control {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
}

.row__control input[type="number"] {
    width: 72px;
    height: 34px;
    text-align: right;
    padding: 0 0.625rem;
    background: color-mix(in srgb, var(--foreground) 4%, var(--background));
    border: 2px solid var(--border);
    border-radius: calc(var(--radius) * 0.7);
    corner-shape: var(--corner-shape);
    color: var(--foreground);
    font-family: var(--font-mono);
    font-size: 0.875rem;
    font-variant-numeric: tabular-nums;
    outline: none;
    transition: border-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.row__control input[type="number"]:focus {
    border-color: color-mix(in srgb, var(--accent) 65%, transparent);
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
    font-size: 0.8125rem;
    color: color-mix(in srgb, var(--foreground) 55%, transparent);
    min-width: 28px;
}

/* Контейнер для Dropdown в row__control — ограничиваем ширину, чтобы он
   не растягивал контрол на всю строку. */
.dd {
    min-width: 160px;
}

/* Volume row растянут — slider занимает основное место. */
.row__control--volume {
    flex: 1;
    min-width: 0;
    gap: 0.75rem;
}

.volume-slider {
    flex: 1;
    min-width: 0;
    height: 4px;
    -webkit-appearance: none;
    appearance: none;
    background: color-mix(in srgb, var(--foreground) 15%, transparent);
    border-radius: 999px;
    outline: none;
    cursor: pointer;
}

.volume-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 999px;
    background: var(--accent);
    cursor: pointer;
    transition: transform 120ms var(--easing-standard);
}

.volume-slider::-webkit-slider-thumb:hover {
    transform: scale(1.15);
}

.volume-slider::-webkit-slider-thumb:active {
    transform: scale(1.25);
}

.row__control--volume .row__unit {
    min-width: 40px;
    text-align: right;
    font-variant-numeric: tabular-nums;
}

.iconbtn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border: 2px solid var(--border);
    background: transparent;
    color: color-mix(in srgb, var(--foreground) 65%, transparent);
    border-radius: calc(var(--radius) * 0.6);
    corner-shape: var(--corner-shape);
    cursor: pointer;
    transition:
        background-color 120ms cubic-bezier(0.2, 0, 0, 1),
        border-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.iconbtn:hover {
    background: color-mix(in srgb, var(--foreground) 8%, transparent);
    color: var(--foreground);
}

/* Toggle живёт по тем же визуальным правилам, что и .row:
   текст слева, capsule справа, border-top отделяет от предыдущей строки. */
.toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.5rem 0;
    cursor: pointer;
    border-top: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
}

.toggle:first-of-type {
    border-top: none;
    padding-top: 0;
}

.toggle input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
}

.toggle__cap {
    position: relative;
    flex-shrink: 0;
    width: 36px;
    height: 20px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--foreground) 18%, transparent);
    transition: background-color 160ms cubic-bezier(0.2, 0, 0, 1);
    order: 2;
}

.toggle__cap::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 999px;
    background: var(--background);
    transition: transform 160ms cubic-bezier(0.2, 0, 0, 1);
}

.toggle input:checked~.toggle__cap {
    background: var(--accent);
}

.toggle input:checked~.toggle__cap::after {
    transform: translateX(16px);
}

.toggle__text {
    flex: 1;
    font-size: 0.9375rem;
    color: var(--foreground);
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
}

.toggle__hint {
    font-size: 0.8125rem;
    color: color-mix(in srgb, var(--foreground) 55%, transparent);
    line-height: 1.35;
}

.settings__foot {
    display: flex;
    justify-content: stretch;
}

/* Reset = такая же кнопка как .pomo__secbtn в PomodoroView (36px, такой же
   padding/border-radius/font), но в destructive-цвете из globals.css. */
.reset {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    width: 100%;
    height: 40px;
    padding: 0 0.875rem;
    background: transparent;
    border: 1px solid color-mix(in srgb, var(--destructive) 55%, transparent);
    border-radius: var(--radius-button);
    corner-shape: var(--corner-shape);
    color: var(--destructive);
    font-family: inherit;
    font-size: 0.8125rem;
    font-weight: 500;
    cursor: pointer;
    transition:
        background-color 160ms cubic-bezier(0.2, 0, 0, 1),
        color 160ms cubic-bezier(0.2, 0, 0, 1),
        border-color 160ms cubic-bezier(0.2, 0, 0, 1);
}

.reset:hover {
    background: color-mix(in srgb, var(--destructive) 12%, transparent);
    border-color: var(--destructive);
}
</style>
