<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { Activity, Dumbbell, Info, Scale } from "@lucide/vue";
import { Button } from "@kosmos/visuals";
import BodyMuscleMap from "./BodyMuscleMap.vue";
import { muscleLabels } from "./body-muscles";

type BodyTab = "development" | "load";
type LoadRange = "week" | "month" | "year";

interface DevelopmentMetric {
  muscle: string;
  score: number;
  level: number;
  bestE1rmKg: number;
  exercise: string;
}

interface LoadMetric {
  muscle: string;
  tonnageKg: number;
  intensity: number;
}

interface BodySnapshot {
  bodyWeightKg: number | null;
  needsBodyWeight: boolean;
  workoutCount: number;
  rangeDays: number;
  development: DevelopmentMetric[];
  load: LoadMetric[];
}

const tab = ref<BodyTab>("development");
const range = ref<LoadRange>("week");
const snapshot = ref<BodySnapshot | null>(null);
const bodyWeight = ref<number | null>(null);
const loading = ref(true);
const savingWeight = ref(false);
const error = ref("");
const selectedMuscle = ref<string | null>(null);

const developmentByMuscle = computed(() =>
  Object.fromEntries((snapshot.value?.development ?? []).map((metric) => [metric.muscle, metric])),
);
const loadByMuscle = computed(() =>
  Object.fromEntries((snapshot.value?.load ?? []).map((metric) => [metric.muscle, metric])),
);

const mapValues = computed<Record<string, number>>(() => {
  const rows = tab.value === "development" ? snapshot.value?.development : snapshot.value?.load;
  if (!rows) return {};
  return Object.fromEntries(
    rows.map((metric) => [metric.muscle, "score" in metric ? metric.score : metric.intensity]),
  );
});

const mapValueLabels = computed<Record<string, string>>(() => {
  if (tab.value === "development") {
    return Object.fromEntries(
      (snapshot.value?.development ?? []).map((metric) => [metric.muscle, `${metric.level} из 5`]),
    );
  }
  return Object.fromEntries(
    (snapshot.value?.load ?? []).map((metric) => [
      metric.muscle,
      `${formatWeight(metric.tonnageKg)} тоннажа`,
    ]),
  );
});

const selectedDevelopment = computed(() =>
  selectedMuscle.value ? developmentByMuscle.value[selectedMuscle.value] : undefined,
);
const selectedLoad = computed(() =>
  selectedMuscle.value ? loadByMuscle.value[selectedMuscle.value] : undefined,
);

function formatWeight(value: number) {
  if (value >= 1000)
    return `${(value / 1000).toLocaleString("ru", { maximumFractionDigits: 1 })} т`;
  return `${value.toLocaleString("ru", { maximumFractionDigits: 0 })} кг`;
}

async function loadSnapshot() {
  loading.value = true;
  error.value = "";
  try {
    snapshot.value = (await window.kepler.ark.request("integrations.body_snapshot", {
      range: range.value,
    })) as BodySnapshot;
    bodyWeight.value = snapshot.value.bodyWeightKg;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Не удалось загрузить данные тела";
  } finally {
    loading.value = false;
  }
}

async function saveBodyWeight() {
  if (bodyWeight.value !== null && (bodyWeight.value < 20 || bodyWeight.value > 400)) {
    error.value = "Укажите вес от 20 до 400 кг";
    return;
  }
  savingWeight.value = true;
  error.value = "";
  try {
    await window.kepler.ark.request("integrations.body_weight_set", {
      bodyWeightKg: bodyWeight.value,
    });
    await loadSnapshot();
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Не удалось сохранить вес";
  } finally {
    savingWeight.value = false;
  }
}

function selectMuscle(muscle: string) {
  selectedMuscle.value = muscle;
}

watch(range, () => {
  if (tab.value === "load") void loadSnapshot();
});

onMounted(loadSnapshot);
</script>

<template>
  <main class="body-page kosmos-scroll">
    <header class="body-header">
      <div>
        <h1>Тело</h1>
        <p>Игровая оценка силы и нагрузки по тренировкам Hevy.</p>
      </div>
      <label class="body-weight">
        <Scale :size="15" />
        <span>Вес</span>
        <input
          v-model.number="bodyWeight"
          type="number"
          min="20"
          max="400"
          step="0.1"
          aria-label="Вес тела в килограммах"
          @keydown.enter="saveBodyWeight"
        />
        <span>кг</span>
        <Button size="sm" variant="ghost" :loading="savingWeight" @click="saveBodyWeight">
          Сохранить
        </Button>
      </label>
    </header>

    <nav class="body-tabs" aria-label="Раздел тела">
      <button type="button" :class="{ active: tab === 'development' }" @click="tab = 'development'">
        <Dumbbell :size="15" /> Развитие
      </button>
      <button type="button" :class="{ active: tab === 'load' }" @click="tab = 'load'">
        <Activity :size="15" /> Нагрузка
      </button>
    </nav>

    <section v-if="tab === 'load'" class="range-tabs" aria-label="Период нагрузки">
      <button
        v-for="option in [
          ['week', 'Неделя'],
          ['month', 'Месяц'],
          ['year', 'Год'],
        ] as const"
        :key="option[0]"
        type="button"
        :class="{ active: range === option[0] }"
        @click="range = option[0]"
      >
        {{ option[1] }}
      </button>
    </section>

    <div v-if="error" class="body-state body-state--error">{{ error }}</div>
    <div v-else-if="loading" class="body-state">Собираю данные тренировок…</div>
    <div v-else-if="!snapshot" class="body-state">Backend Kosmos недоступен.</div>

    <template v-else>
      <aside v-if="snapshot.needsBodyWeight && tab === 'development'" class="body-notice">
        <Info :size="16" />
        Укажите вес тела: без него нельзя посчитать относительную силу.
      </aside>
      <aside
        v-else-if="tab === 'development' && snapshot.development.length === 0"
        class="body-notice"
      >
        <Info :size="16" />
        Нет подходящих силовых подходов. Подключите Hevy в настройках интеграций и получите
        тренировки.
      </aside>
      <aside v-else-if="tab === 'load' && snapshot.workoutCount === 0" class="body-notice">
        <Info :size="16" />
        За этот период нет импортированных тренировок Hevy. Подключите Hevy в настройках интеграций
        или выберите другой период.
      </aside>

      <div class="body-workspace">
        <section class="body-maps" aria-label="Тепловая карта мышц">
          <BodyMuscleMap
            side="front"
            :values="mapValues"
            :value-labels="mapValueLabels"
            :selected="selectedMuscle"
            @select="selectMuscle"
          />
          <BodyMuscleMap
            side="back"
            :values="mapValues"
            :value-labels="mapValueLabels"
            :selected="selectedMuscle"
            @select="selectMuscle"
          />
        </section>

        <aside class="body-details">
          <template v-if="selectedMuscle">
            <span class="body-details__eyebrow">Выбрано</span>
            <h2>{{ muscleLabels[selectedMuscle] ?? selectedMuscle }}</h2>
            <template v-if="tab === 'development'">
              <div class="body-level">
                <strong>{{ selectedDevelopment?.level ?? 0 }}</strong
                ><span>/ 5</span>
              </div>
              <p v-if="selectedDevelopment">
                Лучший расчётный 1ПМ: {{ formatWeight(selectedDevelopment.bestE1rmKg) }}<br />
                {{ selectedDevelopment.exercise }}
              </p>
              <p v-else>Пока нет подходящего силового упражнения.</p>
            </template>
            <template v-else>
              <div class="body-level body-level--load">
                <strong>{{ formatWeight(selectedLoad?.tonnageKg ?? 0) }}</strong>
              </div>
              <p>Суммарный тоннаж за выбранный период.</p>
            </template>
          </template>
          <template v-else>
            <span class="body-details__eyebrow">Подсказка</span>
            <h2>Выберите мышцу</h2>
            <p>Нажмите на область тела или перейдите к ней клавишей Tab.</p>
          </template>

          <div class="body-explanation">
            <Info :size="14" />
            <span v-if="tab === 'development'">
              Уровень 0–5 — игровая оценка лучшего расчётного 1ПМ относительно веса тела. Вторичные
              мышцы получают 60% результата. Это не медицинская оценка.
            </span>
            <span v-else>
              Чем насыщеннее цвет, тем выше тоннаж относительно самой нагруженной мышцы. Вторичные
              мышцы получают половину тоннажа.
            </span>
          </div>
        </aside>
      </div>
    </template>
  </main>
</template>

<style scoped>
.body-page {
  --body-muscle-empty: color-mix(in srgb, var(--foreground) 12%, var(--background));
  height: 100%;
  overflow-y: auto;
  padding: 20px 24px 28px;
  color: var(--foreground);
}

.body-header,
.body-weight,
.body-tabs,
.range-tabs,
.body-notice,
.body-workspace,
.body-maps,
.body-explanation {
  display: flex;
  align-items: center;
}

.body-header {
  justify-content: space-between;
  gap: 18px;
}

.body-header h1 {
  margin: 0;
  font-size: 1.35rem;
}

.body-header p,
.body-details p {
  margin: 4px 0 0;
  color: var(--muted-foreground);
  font-size: 0.75rem;
  line-height: 1.5;
}

.body-weight {
  gap: 7px;
  color: var(--muted-foreground);
  font-size: 0.75rem;
}

.body-weight input {
  width: 68px;
  height: 28px;
  border: 1px solid var(--border);
  border-radius: 7px;
  outline: none;
  background: var(--background);
  color: var(--foreground);
  text-align: right;
}

.body-weight input:focus {
  border-color: var(--accent);
}

.body-tabs {
  gap: 4px;
  margin-top: 18px;
  border-bottom: 1px solid var(--border);
}

.body-tabs button,
.range-tabs button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: 0;
  background: transparent;
  color: var(--muted-foreground);
  font: inherit;
  font-size: 0.75rem;
}

.body-tabs button {
  margin-bottom: -1px;
  padding: 9px 12px;
  border-bottom: 2px solid transparent;
}

.body-tabs button.active {
  border-bottom-color: var(--accent);
  color: var(--foreground);
}

.range-tabs {
  width: fit-content;
  gap: 3px;
  margin-top: 14px;
  padding: 3px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
}

.range-tabs button {
  padding: 5px 10px;
  border-radius: 6px;
}

.range-tabs button.active {
  background: var(--background);
  color: var(--foreground);
  box-shadow: 0 1px 3px color-mix(in srgb, var(--background) 40%, transparent);
}

.body-state,
.body-notice {
  margin-top: 18px;
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px 14px;
  color: var(--muted-foreground);
  font-size: 0.75rem;
}

.body-state--error {
  border-color: color-mix(in srgb, var(--destructive) 40%, var(--border));
  color: var(--destructive);
}

.body-notice {
  gap: 8px;
  background: color-mix(in srgb, var(--accent) 7%, transparent);
}

.body-workspace {
  align-items: stretch;
  gap: 24px;
  margin-top: 18px;
}

.body-maps {
  min-width: 0;
  flex: 1;
  align-items: flex-start;
  gap: 18px;
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 16px;
  background: color-mix(in srgb, var(--foreground) 2%, var(--background));
}

.body-details {
  width: 250px;
  flex: 0 0 250px;
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 18px;
  background: color-mix(in srgb, var(--foreground) 2%, var(--background));
}

.body-details__eyebrow {
  color: var(--muted-foreground);
  font-size: 0.625rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.body-details h2 {
  margin: 5px 0 14px;
  font-size: 1rem;
}

.body-level {
  display: flex;
  align-items: baseline;
  gap: 5px;
}

.body-level strong {
  color: var(--accent);
  font-size: 2.25rem;
  line-height: 1;
}

.body-level span {
  color: var(--muted-foreground);
  font-size: 0.8rem;
}

.body-level--load strong {
  font-size: 1.8rem;
}

.body-explanation {
  align-items: flex-start;
  gap: 7px;
  margin-top: 20px;
  padding-top: 14px;
  border-top: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 0.6875rem;
  line-height: 1.45;
}

.body-explanation svg {
  flex-shrink: 0;
  margin-top: 1px;
}

@media (max-width: 880px) {
  .body-workspace,
  .body-header {
    align-items: stretch;
    flex-direction: column;
  }

  .body-details {
    width: auto;
    flex-basis: auto;
  }
}
</style>
