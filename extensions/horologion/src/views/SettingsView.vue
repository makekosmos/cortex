<script setup lang="ts">
import { computed } from "vue";
import { Play } from "lucide-vue-next";
import { Dropdown, SettingsRow, Toggle } from "@kosmos/visuals";
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
            <SettingsRow title="Рабочее время">
                <template #control>
                    <span class="row__control">
                        <input type="number" min="1" max="180" :value="pomodoroSettings.workMin"
                            @change="(e) => (pomodoroSettings.workMin = clampMin(Number((e.target as HTMLInputElement).value)))" />
                        <span class="row__unit">мин</span>
                    </span>
                </template>
            </SettingsRow>
            <SettingsRow title="Короткий перерыв">
                <template #control>
                    <span class="row__control">
                        <input type="number" min="1" max="60" :value="pomodoroSettings.shortBreakMin"
                            @change="(e) => (pomodoroSettings.shortBreakMin = clampMin(Number((e.target as HTMLInputElement).value)))" />
                        <span class="row__unit">мин</span>
                    </span>
                </template>
            </SettingsRow>
            <SettingsRow title="Длинный перерыв">
                <template #control>
                    <span class="row__control">
                        <input type="number" min="1" max="120" :value="pomodoroSettings.longBreakMin"
                            @change="(e) => (pomodoroSettings.longBreakMin = clampMin(Number((e.target as HTMLInputElement).value)))" />
                        <span class="row__unit">мин</span>
                    </span>
                </template>
            </SettingsRow>
            <SettingsRow title="Помидорок до длинного перерыва">
                <template #control>
                    <span class="row__control">
                        <input type="number" min="1" max="20" :value="pomodoroSettings.pomodorosUntilLongBreak"
                            @change="(e) => (pomodoroSettings.pomodorosUntilLongBreak = clampCount(Number((e.target as HTMLInputElement).value)))" />
                        <span class="row__unit">шт</span>
                    </span>
                </template>
            </SettingsRow>
        </section>

        <section class="group">
            <h3 class="group__title">Поведение</h3>
            <SettingsRow title="Трекать перерывы как «Отдых»">
                <template #control>
                    <Toggle
                        v-model="pomodoroSettings.trackBreaksAsRest"
                        aria-label="Трекать перерывы как «Отдых»"
                    />
                </template>
            </SettingsRow>
            <SettingsRow title="Автостарт рабочего времени">
                <template #control>
                    <Toggle
                        v-model="pomodoroSettings.autoStartWork"
                        aria-label="Автостарт рабочего времени"
                    />
                </template>
            </SettingsRow>
            <SettingsRow title="Автостарт перерыва">
                <template #control>
                    <Toggle
                        v-model="pomodoroSettings.autoStartBreak"
                        aria-label="Автостарт перерыва"
                    />
                </template>
            </SettingsRow>
            <SettingsRow title="Системные уведомления">
                <template #control>
                    <Toggle
                        v-model="pomodoroSettings.systemNotifications"
                        aria-label="Системные уведомления"
                    />
                </template>
            </SettingsRow>
        </section>

        <section class="group">
            <h3 class="group__title">Звуки</h3>
            <SettingsRow title="Конец рабочего времени">
                <template #control>
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
                </template>
            </SettingsRow>
            <SettingsRow title="Конец перерыва">
                <template #control>
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
                </template>
            </SettingsRow>
            <SettingsRow title="Громкость">
                <template #control>
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
                </template>
            </SettingsRow>
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
    padding: 1rem;
}

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

/* SettingsRow приходит из @kosmos/visuals — она уже задаёт layout (title/desc слева,
   control справа) и border-bottom. Здесь только аккуратим padding'и, чтобы строки
   не «торчали» из карточки группы. */
:deep(.kosmos-settings-row) {
    padding: 0.5rem 0;
    border-bottom: none;
    border-top: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
}

:deep(.kosmos-settings-row:first-child) {
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

.dd {
    min-width: 160px;
}

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

.settings__foot {
    display: flex;
    justify-content: stretch;
}

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
