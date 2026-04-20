<template>
  <!-- my-space -->
  <section v-if="activeSpace === 'my-space'" class="spaces-view" data-testid="space-view-my-space">
    <div class="space-empty-block">
      <h3>Открываю заметку пространства</h3>
      <p>Это пространство является обычной заметкой, а не отдельным обзорным экраном.</p>
    </div>
  </section>

  <!-- all-objects -->
  <section
    v-else-if="activeSpace === 'all-objects'"
    class="spaces-view"
    data-testid="space-view-all-objects"
  >
    <div class="space-page-header">
      <div>
        <p class="space-eyebrow">Обзор</p>
        <h1>Все объекты</h1>
        <p class="space-page-text">
          Единая таблица заметок и объектных свойств, отсортированная по последним изменениям.
        </p>
      </div>
    </div>
    <section class="space-panel">
      <div v-if="allObjects.length > 0" class="space-table">
        <div class="space-table-head">
          <span>Название</span>
          <span>Тип</span>
          <span>Обновлено</span>
        </div>
        <button
          v-for="row in allObjects"
          :key="row.id"
          class="space-table-row"
          type="button"
          @click="emit('openEntry', row.id)"
        >
          <span>{{ row.title }}</span>
          <span>{{ row.subtitle }}</span>
          <time>{{ formatDate(row.updatedAt) }}</time>
        </button>
      </div>
      <SpaceEmptyBlock
        v-else
        title="Нет объектов"
        description="Добавь заметки или объектные свойства, чтобы здесь появилась таблица."
        action-label="Создать заметку"
        @action="emit('createEntry')"
      />
    </section>
  </section>

  <!-- all-properties -->
  <section
    v-else-if="activeSpace === 'all-properties'"
    class="spaces-view"
    data-testid="space-view-all-properties"
  >
    <div class="space-page-header">
      <div>
        <p class="space-eyebrow">Модель данных</p>
        <h1>Все свойства</h1>
        <p class="space-page-text">Типы объектов и их свойства.</p>
      </div>
    </div>
    <section class="space-panel">
      <div v-if="propertyTypes.length > 0" class="space-table">
        <div class="space-table-head">
          <span>Тип</span>
          <span>Slug</span>
          <span>Объектов</span>
        </div>
        <div v-for="nt in propertyTypes" :key="nt.id" class="space-table-row">
          <span style="display: flex; align-items: center; gap: 6px">
            <img
              :src="`/anytype/icon/type/default/${nt.icon || 'document'}.svg`"
              alt=""
              width="16"
              height="16"
              draggable="false"
            />
            {{ nt.name }}
          </span>
          <span>{{ nt.slug }}</span>
          <span>{{ entries.filter((e) => e.type_id === nt.id).length }}</span>
        </div>
      </div>
      <SpaceEmptyBlock
        v-else
        title="Нет типов"
        description="Создайте типы объектов в настройках."
      />
    </section>
  </section>

  <!-- all-notes -->
  <section
    v-else-if="activeSpace === 'all-notes'"
    class="spaces-view"
    data-testid="space-view-all-notes"
  >
    <div class="space-page-header">
      <div>
        <p class="space-eyebrow">Коллекция</p>
        <h1>Все заметки</h1>
        <p class="space-page-text">
          Список заметок теперь открывается как отдельная страница, без внутреннего уровня
          пространства в сайдбаре.
        </p>
      </div>
      <div class="space-page-actions">
        <label class="space-sort-control">
          <span>Сортировка</span>
          <select
            :value="sortMode"
            @change="emit('sortModeChange', ($event.target as HTMLSelectElement).value as SortMode)"
          >
            <option value="updated_at">По изменению</option>
            <option value="created_at">По созданию</option>
            <option value="title">По названию</option>
          </select>
        </label>
        <button class="space-primary-btn" type="button" @click="emit('createEntry')">
          Новая заметка
        </button>
      </div>
    </div>
    <div v-if="sortedEntries.length > 0" class="space-table">
      <div class="space-table-head">
        <span>Название</span>
        <span>Тип</span>
        <span>Обновлено</span>
      </div>
      <button
        v-for="entry in sortedEntries"
        :key="entry.id"
        class="space-table-row"
        type="button"
        @click="emit('openEntry', entry.id)"
      >
        <span>{{ getEntryDisplayTitle(entry.title, entry.header_props_json) }}</span>
        <span>{{ entry.type_id ? "С объектным свойством" : "Обычная заметка" }}</span>
        <time>{{ formatDate(entry.updated_at) }}</time>
      </button>
    </div>
    <SpaceEmptyBlock
      v-else
      title="Нет заметок"
      description="Создай первую заметку, и здесь появится полноценная таблица."
      action-label="Создать заметку"
      @action="emit('createEntry')"
    />
  </section>

  <!-- diary -->
  <section v-else-if="activeSpace === 'diary'" class="spaces-view" data-testid="space-view-diary">
    <div class="space-page-header">
      <div>
        <p class="space-eyebrow">Дневник</p>
        <h1>{{ todayFormatted }}</h1>
        <p class="space-page-text">Записи за сегодня и история тренировок.</p>
      </div>
    </div>

    <section class="space-panel">
      <h2 class="diary-section-title" data-testid="diary-today-section">Сегодня</h2>
      <div v-if="todayWorkouts.length > 0" class="space-table">
        <div class="space-table-head">
          <span>Тренировка</span>
          <span>Длительность</span>
          <span>Объём</span>
          <span>Упражнений</span>
        </div>
        <button
          v-for="entry in todayWorkouts"
          :key="entry.id"
          class="space-table-row"
          type="button"
          data-testid="diary-workout-row"
          @click="emit('openEntry', entry.id)"
        >
          <span>{{ entry.title }}</span>
          <span>{{ getHeaderProp(entry, "duration_min") }} мин</span>
          <span>{{ getHeaderProp(entry, "volume_kg") }} кг</span>
          <span>{{ getHeaderProp(entry, "exercise_count") }}</span>
        </button>
      </div>
      <div v-else class="space-empty-block">
        <p>Сегодня тренировок пока нет. Синхронизируйте данные из связанных программ.</p>
      </div>

      <template v-if="todayExercises.length > 0">
        <h3 class="diary-section-subtitle">Упражнения за сегодня</h3>
        <div class="space-table">
          <div class="space-table-head">
            <span>Упражнение</span>
            <span>Мышцы</span>
            <span>Лучший подход</span>
            <span>Объём</span>
          </div>
          <button
            v-for="entry in todayExercises"
            :key="entry.id"
            class="space-table-row"
            type="button"
            data-testid="diary-exercise-row"
            @click="emit('openEntry', entry.id)"
          >
            <span>{{ getHeaderProp(entry, "exercise_name") }}</span>
            <span>{{ getHeaderProp(entry, "muscle_group") }}</span>
            <span>{{ getHeaderProp(entry, "best_set") }}</span>
            <span>{{ getHeaderProp(entry, "total_volume_kg") }} кг</span>
          </button>
        </div>
      </template>
    </section>

    <section class="space-panel">
      <h2 class="diary-section-title" data-testid="diary-history-section">История тренировок</h2>
      <div v-if="allWorkouts.length > 0" class="space-table">
        <div class="space-table-head">
          <span>Тренировка</span>
          <span>Дата</span>
          <span>Длительность</span>
          <span>Объём</span>
        </div>
        <button
          v-for="entry in allWorkouts"
          :key="entry.id"
          class="space-table-row"
          type="button"
          data-testid="diary-history-row"
          @click="emit('openEntry', entry.id)"
        >
          <span>{{ entry.title }}</span>
          <time>{{ getHeaderProp(entry, "date") }}</time>
          <span>{{ getHeaderProp(entry, "duration_min") }} мин</span>
          <span>{{ getHeaderProp(entry, "volume_kg") }} кг</span>
        </button>
      </div>
      <SpaceEmptyBlock
        v-else
        title="Нет тренировок"
        description="Подключите Hevy в настройках (Связанные программы) и синхронизируйте тренировки."
      />
    </section>
  </section>
</template>

<script setup lang="ts">
import { computed, h } from "vue";
import type { SpaceId } from "@/components/sidebar/types";
import { sortEntries, type SortMode } from "@/components/sidebar/types";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { SYSTEM_TYPE_WORKOUT_ID, SYSTEM_TYPE_EXERCISE_ID } from "@/lib/systemTypes";

// Inline empty-block helper using render function (no runtime compiler needed)
const SpaceEmptyBlock = (
  props: { title: string; description: string; actionLabel?: string },
  { emit }: { emit: (event: string) => void },
) =>
  h("div", { class: "space-empty-block" }, [
    h("h3", props.title),
    h("p", props.description),
    props.actionLabel
      ? h(
          "button",
          { class: "space-primary-btn", type: "button", onClick: () => emit("action") },
          props.actionLabel,
        )
      : null,
  ]);

const props = defineProps<{
  activeSpace: SpaceId;
  entries: Entry[];
  noteTypes: NoteType[];
  sortMode: SortMode;
}>();

const emit = defineEmits<{
  sortModeChange: [sortMode: SortMode];
  createEntry: [];
  openEntry: [entryId: string];
}>();

function formatDate(timestamp: number) {
  return new Date(timestamp).toLocaleDateString("ru-RU", { day: "numeric", month: "short" });
}

function getHeaderProp(entry: Entry, key: string): string {
  try {
    const p = JSON.parse(entry.header_props_json || "{}");
    return String(p[key] ?? "");
  } catch {
    return "";
  }
}

function todayDateString(): string {
  return new Date().toISOString().split("T")[0];
}

const sortedEntries = computed(() => sortEntries(props.entries, props.sortMode));

const allObjects = computed(() =>
  props.entries
    .map((entry) => {
      const noteType = entry.type_id ? props.noteTypes.find((nt) => nt.id === entry.type_id) : null;
      return {
        id: entry.id,
        title: getEntryDisplayTitle(entry.title, entry.header_props_json),
        subtitle: noteType?.name ?? "Страница",
        updatedAt: entry.updated_at,
      };
    })
    .sort((a, b) => b.updatedAt - a.updatedAt),
);

const propertyTypes = computed(() => props.noteTypes.filter((nt) => nt.id !== "system-type-page"));

const todayFormatted = computed(() =>
  new Date().toLocaleDateString("ru-RU", {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
  }),
);

const todayWorkouts = computed(() => {
  const today = todayDateString();
  return props.entries
    .filter((e) => e.type_id === SYSTEM_TYPE_WORKOUT_ID && getHeaderProp(e, "date") === today)
    .sort((a, b) => b.created_at - a.created_at);
});

const todayExercises = computed(() =>
  props.entries
    .filter((e) => e.type_id === SYSTEM_TYPE_EXERCISE_ID)
    .filter((e) =>
      todayWorkouts.value.some((w) =>
        e.id.startsWith(`hevy-exercise-${getHeaderProp(w, "hevy_id")}`),
      ),
    ),
);

const allWorkouts = computed(() =>
  props.entries
    .filter((e) => e.type_id === SYSTEM_TYPE_WORKOUT_ID)
    .sort((a, b) => b.created_at - a.created_at),
);
</script>
