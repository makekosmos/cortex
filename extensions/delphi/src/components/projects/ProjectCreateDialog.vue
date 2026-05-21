<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef, watch } from "vue";
import { Check, DollarSign, FolderPlus, X } from "@lucide/vue";

type ProjectColorTag = "red" | "orange" | "yellow" | "green" | "blue" | "purple" | "pink";

export type ProjectCreatePayload = {
  title: string;
  notes: string | null;
  colorTag: ProjectColorTag | null;
  billable: boolean;
  price: number | null;
};

const COLOR_OPTIONS: Array<{
  id: ProjectColorTag;
  label: string;
  className: string;
  softClassName: string;
}> = [
  {
    id: "red",
    label: "Красный",
    className: "bg-red-500",
    softClassName: "text-red-500 ring-red-500/30",
  },
  {
    id: "orange",
    label: "Оранжевый",
    className: "bg-orange-500",
    softClassName: "text-orange-500 ring-orange-500/30",
  },
  {
    id: "yellow",
    label: "Жёлтый",
    className: "bg-yellow-500",
    softClassName: "text-yellow-500 ring-yellow-500/30",
  },
  {
    id: "green",
    label: "Зелёный",
    className: "bg-green-500",
    softClassName: "text-green-500 ring-green-500/30",
  },
  {
    id: "blue",
    label: "Синий",
    className: "bg-blue-500",
    softClassName: "text-blue-500 ring-blue-500/30",
  },
  {
    id: "purple",
    label: "Фиолетовый",
    className: "bg-purple-500",
    softClassName: "text-purple-500 ring-purple-500/30",
  },
  {
    id: "pink",
    label: "Розовый",
    className: "bg-pink-500",
    softClassName: "text-pink-500 ring-pink-500/30",
  },
];

const props = defineProps<{
  open: boolean;
  existingTitles?: string[];
}>();

const emit = defineEmits<{
  "update:open": [open: boolean];
  save: [payload: ProjectCreatePayload];
}>();

const title = shallowRef("");
const notes = shallowRef("");
const colorTag = shallowRef<ProjectColorTag | null>(null);
const billable = shallowRef(false);
const priceInput = shallowRef("");

const titleInputRef = useTemplateRef<HTMLInputElement>("titleInput");

const trimmedTitle = computed(() => title.value.trim());
const canSave = computed(() => trimmedTitle.value.length > 0);

const normalizedExistingTitles = computed(
  () =>
    new Set(
      (props.existingTitles ?? [])
        .map((value) => value.trim().toLocaleLowerCase("ru"))
        .filter((value) => value.length > 0),
    ),
);

const hasDuplicateTitle = computed(() =>
  normalizedExistingTitles.value.has(trimmedTitle.value.toLocaleLowerCase("ru")),
);

function reset() {
  title.value = "";
  notes.value = "";
  colorTag.value = null;
  billable.value = false;
  priceInput.value = "";
}

function close() {
  emit("update:open", false);
}

function handleClose() {
  close();
  reset();
}

function save() {
  if (!canSave.value) return;

  const parsedPrice = priceInput.value.trim() === "" ? null : Number(priceInput.value);
  emit("save", {
    title: trimmedTitle.value,
    notes: notes.value.trim() ? notes.value.trim() : null,
    colorTag: colorTag.value,
    billable: billable.value,
    price: Number.isFinite(parsedPrice) ? (parsedPrice as number) : null,
  });

  close();
  reset();
}

function handleTitleKeydown(event: KeyboardEvent) {
  if (event.key === "Enter") {
    event.preventDefault();
    save();
    return;
  }

  if (event.key === "Escape") {
    event.preventDefault();
    handleClose();
  }
}

function handleNotesKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
    event.preventDefault();
    save();
    return;
  }

  if (event.key === "Escape") {
    event.preventDefault();
    handleClose();
  }
}

watch(
  () => props.open,
  async (isOpen) => {
    if (isOpen) {
      await nextTick();
      titleInputRef.value?.focus();
      titleInputRef.value?.select();
      return;
    }

    reset();
  },
);
</script>

<template>
  <div v-if="open" class="fixed inset-0 z-60 flex items-center justify-center p-4">
    <button
      type="button"
      class="absolute inset-0 bg-black/45 backdrop-blur-sm"
      aria-label="Закрыть создание проекта"
      @click="handleClose"
    />

    <div
      class="relative z-10 w-full max-w-md overflow-hidden rounded-[1.5rem] border border-(--border) bg-(--popover) shadow-2xl"
    >
      <div class="flex items-start gap-3 border-b border-(--border) px-5 pb-4 pt-5">
        <div
          class="flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl bg-(--primary)/12 text-(--primary)"
        >
          <FolderPlus :size="18" />
        </div>

        <div class="min-w-0 flex-1">
          <h2 class="text-base font-semibold text-(--foreground)">Новый проект</h2>
          <p class="mt-1 text-sm text-(--muted-foreground)">
            Создай проект и сразу перейди внутрь него.
          </p>
        </div>

        <button
          type="button"
          class="rounded-xl p-2 text-(--muted-foreground) transition-colors hover:bg-(--secondary) hover:text-(--foreground)"
          title="Закрыть"
          @click="handleClose"
        >
          <X :size="16" />
        </button>
      </div>

      <div class="flex flex-col gap-4 px-5 py-5">
        <div class="flex flex-col gap-2">
          <label
            for="project-create-title"
            class="text-xs font-semibold uppercase tracking-[0.18em] text-(--muted-foreground)"
          >
            Название
          </label>
          <input
            id="project-create-title"
            ref="titleInput"
            v-model="title"
            type="text"
            placeholder="Например, Новый продукт"
            class="w-full rounded-2xl border border-(--border) bg-(--background) px-4 py-3 text-sm text-(--foreground) outline-none transition-colors placeholder:text-(--muted-foreground)/70 focus:border-blue-500/60"
            @keydown="handleTitleKeydown"
          />
          <p class="text-xs text-(--muted-foreground)">
            <template v-if="hasDuplicateTitle">
              Проект с таким названием уже есть. Создание всё равно доступно.
            </template>
            <template v-else> `Enter` создаёт проект сразу. </template>
          </p>
        </div>

        <div class="flex flex-col gap-2">
          <label
            for="project-create-notes"
            class="text-xs font-semibold uppercase tracking-[0.18em] text-(--muted-foreground)"
          >
            Описание
          </label>
          <textarea
            id="project-create-notes"
            v-model="notes"
            rows="4"
            placeholder="Коротко опиши смысл проекта или следующий шаг"
            class="w-full resize-none rounded-2xl border border-(--border) bg-(--background) px-4 py-3 text-sm text-(--foreground) outline-none transition-colors placeholder:text-(--muted-foreground)/70 focus:border-blue-500/60"
            @keydown="handleNotesKeydown"
          />
          <p class="text-xs text-(--muted-foreground)">
            `Ctrl/Cmd + Enter` создаёт проект из поля описания.
          </p>
        </div>

        <div class="flex flex-col gap-2">
          <span class="text-xs font-semibold uppercase tracking-[0.18em] text-(--muted-foreground)">
            Цвет
          </span>

          <div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
            <button
              type="button"
              :class="[
                'flex items-center gap-2 rounded-2xl border px-3 py-2.5 text-sm transition-colors',
                colorTag === null
                  ? 'border-(--foreground) bg-(--secondary) text-(--foreground)'
                  : 'border-(--border) bg-(--background) text-(--muted-foreground) hover:bg-(--secondary)',
              ]"
              @click="colorTag = null"
            >
              <span class="h-2.5 w-2.5 rounded-full bg-(--muted-foreground)" />
              <span class="truncate">Без цвета</span>
              <Check v-if="colorTag === null" :size="14" class="ml-auto" />
            </button>

            <button
              v-for="option in COLOR_OPTIONS"
              :key="option.id"
              type="button"
              :class="[
                'flex items-center gap-2 rounded-2xl border px-3 py-2.5 text-sm transition-colors',
                colorTag === option.id
                  ? ['bg-(--secondary)', option.softClassName, 'border-current']
                  : 'border-(--border) bg-(--background) text-(--muted-foreground) hover:bg-(--secondary)',
              ]"
              @click="colorTag = option.id"
            >
              <span :class="['h-2.5 w-2.5 rounded-full', option.className]" />
              <span class="truncate">{{ option.label }}</span>
              <Check v-if="colorTag === option.id" :size="14" class="ml-auto" />
            </button>
          </div>
        </div>

        <div class="flex flex-col gap-2">
          <span class="text-xs font-semibold uppercase tracking-[0.18em] text-(--muted-foreground)">
            Оплата
          </span>

          <div class="flex flex-wrap items-center gap-2">
            <button
              type="button"
              :class="[
                'flex items-center gap-2 rounded-2xl border px-3 py-2 text-sm transition-colors',
                billable
                  ? 'border-emerald-500/40 bg-emerald-500/12 text-emerald-500'
                  : 'border-(--border) bg-(--background) text-(--muted-foreground) hover:bg-(--secondary)',
              ]"
              @click="billable = !billable"
            >
              <DollarSign :size="14" />
              <span>Оплачиваемый</span>
              <Check v-if="billable" :size="14" class="ml-1" />
            </button>

            <label
              v-if="billable"
              class="flex flex-1 items-center gap-2 rounded-2xl border border-(--border) bg-(--background) px-3 py-2 text-sm text-(--foreground)"
            >
              <span class="text-(--muted-foreground)">Бюджет</span>
              <input
                v-model="priceInput"
                type="number"
                inputmode="decimal"
                min="0"
                step="0.01"
                placeholder="0.00"
                class="w-full min-w-0 bg-transparent text-sm outline-none placeholder:text-(--muted-foreground)/60"
              />
            </label>
          </div>
          <p class="text-xs text-(--muted-foreground)">
            Если включено, задачи внутри проекта по умолчанию оплачиваемые.
          </p>
        </div>
      </div>

      <div class="flex items-center justify-end gap-2 border-t border-(--border) px-5 py-4">
        <button
          type="button"
          class="rounded-2xl px-4 py-2 text-sm font-medium text-(--muted-foreground) transition-colors hover:bg-(--secondary) hover:text-(--foreground)"
          @click="handleClose"
        >
          Отмена
        </button>

        <button
          type="button"
          :disabled="!canSave"
          :class="[
            'rounded-2xl px-4 py-2 text-sm font-semibold transition-opacity',
            canSave
              ? 'bg-(--foreground) text-(--background) hover:opacity-90'
              : 'cursor-not-allowed bg-(--secondary) text-(--muted-foreground)',
          ]"
          @click="save"
        >
          Создать проект
        </button>
      </div>
    </div>
  </div>
</template>
