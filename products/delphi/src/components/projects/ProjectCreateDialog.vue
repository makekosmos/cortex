<script setup lang="ts">
import { computed, nextTick, shallowRef, watch } from "vue";
import { Check, FolderPlus, X } from "@lucide/vue";
import { Button, IconButton, Modal, Textarea, TextInput, Toggle } from "@kosmos/visuals";

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
}> = [
  { id: "red", label: "Красный", className: "project-create__color--red" },
  { id: "orange", label: "Оранжевый", className: "project-create__color--orange" },
  { id: "yellow", label: "Жёлтый", className: "project-create__color--yellow" },
  { id: "green", label: "Зелёный", className: "project-create__color--green" },
  { id: "blue", label: "Синий", className: "project-create__color--blue" },
  { id: "purple", label: "Фиолетовый", className: "project-create__color--purple" },
  { id: "pink", label: "Розовый", className: "project-create__color--pink" },
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
      const titleInput = document.getElementById("project-create-title") as HTMLInputElement | null;
      titleInput?.focus();
      titleInput?.select();
      return;
    }

    reset();
  },
);
</script>

<template>
  <Modal :open="open" width="min(448px, 92vw)" hide-close @close="handleClose">
    <template #header>
      <div class="project-create__header">
        <div class="project-create__header-icon">
          <FolderPlus :size="18" />
        </div>

        <div class="min-w-0 flex-1">
          <h2 class="project-create__title">Новый проект</h2>
          <p class="project-create__subtitle">Создай проект и сразу перейди внутрь него.</p>
        </div>

        <IconButton :size="32" :radius="16" title="Закрыть" @click="handleClose">
          <X :size="16" />
        </IconButton>
      </div>
    </template>

    <div class="project-create__body">
      <div class="flex flex-col gap-2">
        <label for="project-create-title" class="project-create__label"> Название </label>
        <TextInput
          id="project-create-title"
          v-model="title"
          type="text"
          placeholder="Например, Новый продукт"
          @keydown="handleTitleKeydown"
        />
        <p class="project-create__hint">
          <template v-if="hasDuplicateTitle">
            Проект с таким названием уже есть. Создание всё равно доступно.
          </template>
          <template v-else> `Enter` создаёт проект сразу. </template>
        </p>
      </div>

      <div class="flex flex-col gap-2">
        <label for="project-create-notes" class="project-create__label"> Описание </label>
        <Textarea
          id="project-create-notes"
          v-model="notes"
          :rows="4"
          :min-height="96"
          resize="none"
          placeholder="Коротко опиши смысл проекта или следующий шаг"
          @keydown="handleNotesKeydown"
        />
        <p class="project-create__hint">`Ctrl/Cmd + Enter` создаёт проект из поля описания.</p>
      </div>

      <div class="flex flex-col gap-2">
        <span class="project-create__label"> Цвет </span>

        <div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
          <button
            type="button"
            :class="[
              'project-create__color-option',
              colorTag === null
                ? 'project-create__color-option--selected'
                : 'project-create__color-option--idle',
            ]"
            @click="colorTag = null"
          >
            <span class="project-create__color-dot project-create__color--none" />
            <span class="truncate">Без цвета</span>
            <Check v-if="colorTag === null" :size="14" class="ml-auto" />
          </button>

          <button
            v-for="option in COLOR_OPTIONS"
            :key="option.id"
            type="button"
            :class="[
              'project-create__color-option',
              option.className,
              colorTag === option.id
                ? 'project-create__color-option--selected'
                : 'project-create__color-option--idle',
            ]"
            @click="colorTag = option.id"
          >
            <span class="project-create__color-dot" />
            <span class="truncate">{{ option.label }}</span>
            <Check v-if="colorTag === option.id" :size="14" class="ml-auto" />
          </button>
        </div>
      </div>

      <div class="flex flex-col gap-2">
        <span class="project-create__label"> Оплата </span>

        <div class="project-create__billing">
          <Toggle v-model="billable" label="Оплачиваемый" />

          <label v-if="billable" class="project-create__price">
            <span class="project-create__price-label">Бюджет</span>
            <TextInput
              v-model="priceInput"
              type="text"
              inputmode="decimal"
              placeholder="0.00"
              size="sm"
            />
          </label>
        </div>
        <p class="project-create__hint">
          Если включено, задачи внутри проекта по умолчанию оплачиваемые.
        </p>
      </div>
    </div>

    <template #footer>
      <Button variant="ghost" @click="handleClose">Отмена</Button>
      <Button :disabled="!canSave" @click="save">Создать проект</Button>
    </template>
  </Modal>
</template>

<style scoped>
.project-create__header {
  display: flex;
  width: 100%;
  align-items: flex-start;
  gap: var(--space-1);
}

.project-create__header-icon {
  display: flex;
  width: var(--size-control-md);
  height: var(--size-control-md);
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-button);
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}

.project-create__title {
  margin: 0;
  color: var(--foreground);
  font-size: var(--kosmos-text-subheading-size);
  font-weight: var(--kosmos-text-subheading-weight);
  line-height: var(--kosmos-text-subheading-line-height);
}

.project-create__subtitle {
  margin: 4px 0 0;
  color: var(--muted-foreground);
  font-size: var(--kosmos-text-body-size);
  line-height: 1.45;
}

.project-create__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.project-create__label {
  color: var(--muted-foreground);
  font-size: var(--kosmos-text-caption-size);
  font-weight: 600;
  letter-spacing: 0;
  text-transform: uppercase;
}

.project-create__hint {
  color: var(--muted-foreground);
  font-size: var(--kosmos-text-caption-size);
}

.project-create__color-option {
  display: flex;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-button);
  padding: 10px 12px;
  color: var(--muted-foreground);
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  font-size: var(--kosmos-text-body-size);
  transition:
    background-color 140ms var(--easing-standard),
    border-color 140ms var(--easing-standard),
    color 140ms var(--easing-standard);
}

.project-create__color-option:hover {
  background: var(--secondary);
}

.project-create__color-option--selected {
  border-color: var(--project-color, var(--foreground));
  color: var(--project-color, var(--foreground));
  background: color-mix(in srgb, var(--project-color, var(--foreground)) 12%, transparent);
}

.project-create__color-option--idle {
  border-color: var(--border);
}

.project-create__color-dot {
  width: 10px;
  height: 10px;
  flex-shrink: 0;
  border-radius: var(--radius-pill, 999px);
  background: var(--project-color, var(--muted-foreground));
}

.project-create__color--none {
  --project-color: var(--muted-foreground);
}

.project-create__color--red {
  --project-color: var(--destructive);
}

.project-create__color--orange {
  --project-color: color-mix(in srgb, var(--status-warning) 78%, var(--destructive));
}

.project-create__color--yellow {
  --project-color: var(--status-warning);
}

.project-create__color--green {
  --project-color: var(--status-success);
}

.project-create__color--blue {
  --project-color: var(--accent);
}

.project-create__color--purple {
  --project-color: color-mix(in srgb, var(--accent) 70%, var(--destructive));
}

.project-create__color--pink {
  --project-color: color-mix(in srgb, var(--destructive) 65%, var(--accent));
}

.project-create__billing {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-1);
}

.project-create__price {
  display: grid;
  min-width: min(100%, 220px);
  flex: 1;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: center;
  gap: var(--space-1);
}

.project-create__price-label {
  color: var(--muted-foreground);
  font-size: var(--kosmos-text-body-size);
}
</style>
