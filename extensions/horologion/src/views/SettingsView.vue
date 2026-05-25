<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Play } from "@lucide/vue";
import { Dropdown, SettingsRow, SettingsToggleRow } from "@kosmos/visuals";
import SettingsNumberRow from "../components/SettingsNumberRow.vue";
import SoundPickerRow from "../components/SoundPickerRow.vue";
import { pomodoroSettings, resetPomodoroSettings } from "../lib/pomodoroSettings";
import { playSound, SOUND_OPTIONS, type SoundName } from "../lib/sounds";

const soundOptions = computed(() => SOUND_OPTIONS.map((s) => ({ value: s.value, label: s.label })));

interface Blocklist {
  id: string;
  name: string;
}
const blocklists = ref<Blocklist[]>([]);
const blocklistsAvailable = ref(false);

onMounted(async () => {
  const k = (
    window as unknown as {
      kepler?: {
        ark?: { request: (op: string, params?: Record<string, unknown>) => Promise<unknown> };
      };
    }
  ).kepler;
  if (!k?.ark) return;
  try {
    const res = (await k.ark.request("focus.list_blocklists")) as
      | { blocklists?: Blocklist[] }
      | Blocklist[];
    const list = Array.isArray(res) ? res : (res?.blocklists ?? []);
    blocklists.value = list.filter(
      (b) => b && typeof b.id === "string" && typeof b.name === "string",
    );
    blocklistsAvailable.value = true;
  } catch (e) {
    blocklistsAvailable.value = false;
    console.warn("[horologion] focus.list_blocklists unavailable:", e);
  }
});

const blocklistOptions = computed(() => [
  { value: "", label: "Без блокировки" },
  ...blocklists.value.map((b) => ({ value: b.id, label: b.name })),
]);

const blocklistModel = computed<string>({
  get: () => pomodoroSettings.focusBlocklistId ?? "",
  set: (v: string) => {
    pomodoroSettings.focusBlocklistId = v === "" ? null : v;
  },
});

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
      <SettingsNumberRow
        v-model="pomodoroSettings.workMin"
        title="Рабочее время"
        :min="1"
        :max="180"
        :clamp="clampMin"
        unit="мин"
      />
      <SettingsNumberRow
        v-model="pomodoroSettings.shortBreakMin"
        title="Короткий перерыв"
        :min="1"
        :max="60"
        :clamp="clampMin"
        unit="мин"
      />
      <SettingsNumberRow
        v-model="pomodoroSettings.longBreakMin"
        title="Длинный перерыв"
        :min="1"
        :max="120"
        :clamp="clampMin"
        unit="мин"
      />
      <SettingsNumberRow
        v-model="pomodoroSettings.pomodorosUntilLongBreak"
        title="Помидорок до длинного перерыва"
        :min="1"
        :max="20"
        :clamp="clampCount"
        unit="шт"
      />
    </section>

    <section class="group">
      <h3 class="group__title">Поведение</h3>
      <SettingsToggleRow
        v-model="pomodoroSettings.trackBreaksAsRest"
        title="Трекать перерывы как «Отдых»"
      />
      <SettingsToggleRow
        v-model="pomodoroSettings.autoStartWork"
        title="Автостарт рабочего времени"
      />
      <SettingsToggleRow v-model="pomodoroSettings.autoStartBreak" title="Автостарт перерыва" />
      <SettingsToggleRow
        v-model="pomodoroSettings.systemNotifications"
        title="Системные уведомления"
      />
      <SettingsRow
        v-if="blocklistsAvailable"
        title="Блокировки во время фокуса"
        description="Профиль блоклиста, активируется на work-фазе помодоро"
      >
        <template #control>
          <span class="row__control">
            <Dropdown v-model="blocklistModel" :options="blocklistOptions" class="dd" />
          </span>
        </template>
      </SettingsRow>
    </section>

    <section class="group">
      <h3 class="group__title">Звуки</h3>
      <SoundPickerRow
        v-model="pomodoroSettings.workEndSound"
        title="Конец рабочего времени"
        :options="soundOptions"
        @preview="testSound"
      />
      <SoundPickerRow
        v-model="pomodoroSettings.breakEndSound"
        title="Конец перерыва"
        :options="soundOptions"
        @preview="testSound"
      />
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
              @input="
                (e) =>
                  (pomodoroSettings.ringtoneVolume =
                    Number((e.target as HTMLInputElement).value) / 100)
              "
            />
            <span class="row__unit">{{ Math.round(pomodoroSettings.ringtoneVolume * 100) }}%</span>
            <button
              type="button"
              class="iconbtn"
              title="Прослушать"
              @click="testSound(pomodoroSettings.workEndSound)"
            >
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

/* .row__control / .row__unit input-number стили переехали в
   `components/SettingsNumberRow.vue`. Здесь оставлены только те объявления,
   что нужны прочим использованиям: dropdown'ы и volume-slider в звуках. */
.row__control {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
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
