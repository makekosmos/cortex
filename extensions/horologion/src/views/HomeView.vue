<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import PomodoroView from "./PomodoroView.vue";
import StopwatchView from "./StopwatchView.vue";
import ListView from "./ListView.vue";
import PomodoroDraftInput from "../components/PomodoroDraftInput.vue";
import { entriesChangedAt, pomodoroDraft, timerMode, type PomodoroDraftTask } from "../lib/store";
import { usePomodoroSession as usePomodoro } from "../lib/usePomodoroSession";

const route = useRoute();

// Deep links из Kepler launcher: `?mode=pomodoro|stopwatch` переключает таб.
// Подписан и при mount, и реактивно — повторный invoke "Помодоро" / "Секундомер"
// при уже открытом окне меняет режим.
function applyModeQuery(value: unknown) {
  if (value === "pomodoro" || value === "stopwatch") {
    timerMode.value = value;
  }
}
watch(
  () => route.query.mode,
  (mode) => applyModeQuery(mode),
  { immediate: true },
);

const p = usePomodoro();

// Направление swipe-анимации зависит от перехода:
// pomodoro → stopwatch — справа налево (forward)
// stopwatch → pomodoro — слева направо (backward)
const swipeDir = ref<"fwd" | "bwd">("fwd");
const transitionName = computed(() => (swipeDir.value === "fwd" ? "swipe-fwd" : "swipe-bwd"));
// JS-driven height transition card'а. Логика:
//   1. На клик меняется timerMode → watcher фиксирует текущую высоту в inline-style.
//   2. await nextTick — DOM обновляется, transition контента уезжает параллельно.
//   3. Снимаем lock (height: auto) — измеряем natural-высоту нового контента.
//   4. Возвращаем oldH → форсируем reflow → запускаем CSS transition 420мс к newH.
//   5. После 460мс убираем inline-стили — card возвращается к auto.
const cardRef = ref<HTMLElement | null>(null);

async function runHeightTransition(): Promise<void> {
  const card = cardRef.value;
  if (!card) return;
  const oldH = card.offsetHeight;
  card.style.height = `${oldH}px`;
  card.style.overflow = "hidden";

  // Без задержки: сразу даём DOM обновиться, измеряем новую высоту и
  // запускаем transition. Параллельно идёт swipe внутреннего контента.
  await nextTick();
  card.style.height = "auto";
  const newH = card.offsetHeight;
  if (oldH === newH) {
    card.style.height = "";
    card.style.overflow = "";
    return;
  }

  card.style.height = `${oldH}px`;
  void card.offsetHeight;
  card.style.transition = "height 420ms cubic-bezier(0.4, 0, 0.2, 1)";
  card.style.height = `${newH}px`;

  // Ждём реального завершения CSS-перехода `height`, а не hardcoded delay —
  // setTimeout(460) рейсится с фактическим transition и даёт jank (контент
  // дёргается, если фактический transition длится дольше). Listener
  // снимается на transitionend ИЛИ transitioncancel; safety-таймаут 800мс
  // на случай, если ни одно событие не пришло (например, transition был
  // удалён извне).
  await new Promise<void>((resolve) => {
    const handler = (e: TransitionEvent) => {
      if (e.target !== card || e.propertyName !== "height") return;
      card.removeEventListener("transitionend", handler);
      card.removeEventListener("transitioncancel", handler);
      resolve();
    };
    card.addEventListener("transitionend", handler);
    card.addEventListener("transitioncancel", handler);
    setTimeout(() => {
      card.removeEventListener("transitionend", handler);
      card.removeEventListener("transitioncancel", handler);
      resolve();
    }, 800);
  });
  card.style.transition = "";
  card.style.height = "";
  card.style.overflow = "";
}

watch(timerMode, (next, prev) => {
  // pomodoro → stopwatch: контент уезжает ВПРАВО, новый приходит СЛЕВА (bwd).
  // stopwatch → pomodoro: контент уезжает ВЛЕВО, новый приходит СПРАВА (fwd).
  swipeDir.value = prev === "pomodoro" && next === "stopwatch" ? "bwd" : "fwd";
  void runHeightTransition();
});

// Активна ли сессия. Pomodoro считается активной по `p.isRunning` (фаза
// work/break и таймер тикает). Stopwatch — по наличию running time_entry
// (которое stopwatch создаёт через startTimer). Используем
// `entriesChangedAt` как тригер обновления — listRunning перечитывается.
const runningExists = ref(false);
async function refreshRunning() {
  const list = await window.horologion.timeEntries.listRunning();
  runningExists.value = list.length > 0;
}
onMounted(refreshRunning);
watch(entriesChangedAt, refreshRunning);

const isSessionActive = computed(() => p.isRunning.value || runningExists.value);

const draftV = computed({
  get: () => pomodoroDraft.value.title,
  set: (v) => {
    pomodoroDraft.value = { ...pomodoroDraft.value, title: v };
  },
});
const draftTasks = computed({
  get: () => pomodoroDraft.value.tasks,
  set: (v: PomodoroDraftTask[]) => {
    pomodoroDraft.value = { ...pomodoroDraft.value, tasks: v };
  },
});

async function onSubmit() {
  if (p.isRunning.value) {
    if (p.isPaused.value) p.resume();
    else p.pause();
    return;
  }
  await p.start({
    title: pomodoroDraft.value.title,
    tasks: pomodoroDraft.value.tasks.slice(),
  });
}
</script>

<template>
  <div class="home">
    <!-- Группа draft + pomo. При isSessionActive они сжимаются вплотную
             друг к другу: gap → 0 + соседние углы выпрямляются → визуально
             образуют единое тело. -->
    <div class="home__top" :class="{ 'home__top--connected': isSessionActive }">
      <section class="home__draft">
        <PomodoroDraftInput
          v-model="draftV"
          v-model:tasks="draftTasks"
          placeholder="Над чем работаем? @ для задачи"
          @submit="onSubmit"
        />
      </section>

      <!-- Верхний прямоугольник — фокусная зона. Переключатель режима
                 (Помодоро / Секундомер) сверху, ниже выбранный таймер. -->
      <section ref="cardRef" class="home__pomo" :class="{ 'home__pomo--session': isSessionActive }">
        <div class="mode-toggle" role="tablist" aria-label="Режим таймера">
          <!-- Pill сидит абсолютно, transform-translateX анимирует
                     переезд между двумя слотами при смене timerMode.
                     Когда сессия активна — один из кнопок ужимается до 0,
                     pill остаётся под активным. -->
          <span
            class="mode-toggle__pill"
            :class="`mode-toggle__pill--${timerMode}`"
            aria-hidden="true"
          />
          <button
            type="button"
            class="mode-toggle__btn"
            :class="{
              'mode-toggle__btn--active': timerMode === 'pomodoro',
              'mode-toggle__btn--hidden': isSessionActive && timerMode !== 'pomodoro',
            }"
            role="tab"
            :aria-selected="timerMode === 'pomodoro'"
            :tabindex="isSessionActive && timerMode !== 'pomodoro' ? -1 : 0"
            @click="timerMode = 'pomodoro'"
          >
            Помодоро
          </button>
          <button
            type="button"
            class="mode-toggle__btn"
            :class="{
              'mode-toggle__btn--active': timerMode === 'stopwatch',
              'mode-toggle__btn--hidden': isSessionActive && timerMode !== 'stopwatch',
            }"
            role="tab"
            :aria-selected="timerMode === 'stopwatch'"
            :tabindex="isSessionActive && timerMode !== 'stopwatch' ? -1 : 0"
            @click="timerMode = 'stopwatch'"
          >
            Секундомер
          </button>
        </div>

        <div class="home__pomo-stage">
          <transition :name="transitionName">
            <PomodoroView v-if="timerMode === 'pomodoro'" key="pomodoro" />
            <StopwatchView v-else key="stopwatch" />
          </transition>
        </div>
      </section>
    </div>

    <!-- Нижняя часть — список записей дня/недели. -->
    <section class="home__list">
      <ListView />
    </section>
  </div>
</template>

<style scoped>
.home {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1rem 0;
}

/* Группа draft + pomo с заметным отступом между ними. При --connected gap
   уменьшается до 0 и нижние углы draft + верхние pomo выпрямляются —
   плавная анимация делает «соединение» в единое тело. */
.home__top {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  transition: gap 420ms cubic-bezier(0.4, 0, 0.2, 1);
}

.home__top--connected {
  gap: 0;
}

/* В .connected состоянии draft и pomo прилегают друг к другу;
   соседние углы выпрямляются. */
.home__top--connected .home__draft {
  border-bottom-left-radius: 0;
  border-bottom-right-radius: 0;
  border-bottom-color: transparent;
}

.home__top--connected .home__pomo {
  border-top-left-radius: 0;
  border-top-right-radius: 0;
  border-top-color: transparent;
}

.home__pomo {
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  align-items: stretch;
  gap: 0.75rem;
  padding: 1rem 1rem 0.75rem;
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 3);
  corner-shape: var(--corner-shape);
  transition:
    border-top-left-radius 420ms cubic-bezier(0.4, 0, 0.2, 1),
    border-top-right-radius 420ms cubic-bezier(0.4, 0, 0.2, 1),
    border-top-color 420ms cubic-bezier(0.4, 0, 0.2, 1);
  /* Высоту анимирует runHeightTransition в скрипте. */
}

/* Stage = wrapper для transition. position: relative чтобы leaving элемент
   мог быть position: absolute и не контрибутил в высоту → высота этой
   секции равна entering элементу, и .home__pomo (родительский flex)
   автоматически подстраивается через interpolate-size. */
.home__pomo-stage {
  position: relative;
}

/* Segmented control — переключатель Помодоро ↔ Секундомер.
   min-width фиксирует ширину шайбы, чтобы pill при расширении до full-width
   и неактивная кнопка при max-width:0 НЕ ужимали саму шайбу. Это устраняет
   резкий snap в конце анимации (calc 100%-8px стабилен относительно
   фиксированной ширины). */
.mode-toggle {
  position: relative;
  display: flex;
  align-self: center;
  min-width: 224px;
  padding: 4px;
  background: var(--background);
  border: 1px solid var(--border);
  border-radius: 999px;
}

/* Все свойства шайбы и кнопок — синхронная плавная анимация одним
   easing'ом без delays. Пользователь видит «единое» движение, без
   sequential рывков. */
.mode-toggle__pill {
  position: absolute;
  top: 4px;
  bottom: 4px;
  left: 4px;
  width: calc(50% - 4px);
  background: var(--accent);
  border-radius: 999px;
  transition:
    transform 420ms cubic-bezier(0.4, 0, 0.2, 1),
    width 420ms cubic-bezier(0.4, 0, 0.2, 1);
  pointer-events: none;
  z-index: 0;
}

.mode-toggle__pill--pomodoro {
  transform: translateX(0);
}

.mode-toggle__pill--stopwatch {
  transform: translateX(100%);
}

/* Сессия активна — pill расширяется на весь toggle (min-width: 224px
   стабилен, calc(100% - 8px) не «дрожит» в конце). */
.home__pomo--session .mode-toggle__pill {
  width: calc(100% - 8px);
  transform: translateX(0);
}

.mode-toggle__btn {
  position: relative;
  z-index: 1;
  flex: 1 1 0;
  min-width: 0;
  /* max-width нужен для transition к 0 (none не транзишнится). */
  max-width: 200px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 28px;
  padding: 0 0.875rem;
  background: transparent;
  border: none;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
  border-radius: 999px;
  font-family: inherit;
  font-size: 0.8125rem;
  font-weight: 600;
  cursor: pointer;
  overflow: hidden;
  white-space: nowrap;
  transition:
    color 200ms cubic-bezier(0.2, 0, 0, 1),
    max-width 420ms cubic-bezier(0.4, 0, 0.2, 1),
    padding 420ms cubic-bezier(0.4, 0, 0.2, 1),
    opacity 420ms cubic-bezier(0.4, 0, 0.2, 1);
}

.mode-toggle__btn:hover {
  color: var(--foreground);
}

.mode-toggle__btn--active,
.mode-toggle__btn--active:hover {
  color: var(--accent-foreground);
}

/* Сессия запущена — неактивная кнопка ужимается (max-width 0 + padding 0
   + opacity 0). Шайба остаётся min-width: 224px (не ужимается), активная
   кнопка через flex-grow занимает всё доступное пространство, и pill
   плавно расширяется до full-width. */
.mode-toggle__btn--hidden {
  max-width: 0;
  padding: 0;
  opacity: 0;
  pointer-events: none;
}

/* Swipe-анимации. Уходящий элемент идёт в position: absolute (чтобы не
   контрибутить в высоту stage'а — высота card'а сразу видит entering size
   и интерполирует через interpolate-size). Forward = pomodoro → stopwatch
   (справа налево), backward = обратно. */
.swipe-fwd-leave-active,
.swipe-bwd-leave-active {
  position: absolute;
  inset: 0;
}

.swipe-fwd-enter-active,
.swipe-fwd-leave-active,
.swipe-bwd-enter-active,
.swipe-bwd-leave-active {
  transition:
    opacity 280ms cubic-bezier(0.4, 0, 0.2, 1),
    transform 320ms cubic-bezier(0.4, 0, 0.2, 1);
  will-change: transform, opacity;
}

/* Forward: помодоро → секундомер (справа налево) */
.swipe-fwd-enter-from {
  opacity: 0;
  transform: translateX(48px);
}

.swipe-fwd-leave-to {
  opacity: 0;
  transform: translateX(-48px);
}

/* Backward: секундомер → помодоро (слева направо) */
.swipe-bwd-enter-from {
  opacity: 0;
  transform: translateX(-48px);
}

.swipe-bwd-leave-to {
  opacity: 0;
  transform: translateX(48px);
}

/* Card для поля ввода — те же стиль/радиус что и .home__pomo, но плотнее.
   position: relative нужно для абсолютно-позиционированного .home__bridge,
   который висит на нижнем крае card'а. */
.home__draft {
  position: relative;
  padding: 0.25rem;
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 3);
  corner-shape: var(--corner-shape);
  transition:
    border-bottom-left-radius 420ms cubic-bezier(0.4, 0, 0.2, 1),
    border-bottom-right-radius 420ms cubic-bezier(0.4, 0, 0.2, 1),
    border-bottom-color 420ms cubic-bezier(0.4, 0, 0.2, 1);
}

/* Внутреннее скруглённое поле PomodoroDraftInput оставляем со своим
   border'ом и border-radius (видим как input), но фон должен быть тем же
   что и у обёртки .home__draft — никакого видимого «прямоугольника в
   прямоугольнике». */
.home__draft :deep(.pdi__row) {
  background: var(--background);
}
</style>
