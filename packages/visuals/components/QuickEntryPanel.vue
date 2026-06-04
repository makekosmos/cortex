<script setup lang="ts">
import { shallowRef, watch, nextTick, useTemplateRef, onBeforeUnmount } from "vue";
import { DollarSign, Folder, X } from "@lucide/vue";
import DateChip from "./DateChip.vue";
import type { QuickEntryProject, QuickEntrySavePayload } from "./types";

const props = defineProps<{
  open: boolean;
  projects?: QuickEntryProject[];
  defaultScheduledDate?: string | null;
  defaultProjectId?: string | null;
}>();

const emit = defineEmits<{
  "update:open": [boolean];
  save: [payload: QuickEntrySavePayload];
}>();

const title = shallowRef("");
const notes = shallowRef("");
const scheduledDate = shallowRef<string | null>(null);
const selectedProjectId = shallowRef<string | null>(null);
const billable = shallowRef(false);
const priceInput = shallowRef("");
const showProjectMenu = shallowRef(false);

const titleRef = useTemplateRef<HTMLInputElement>("titleInput");
const projectMenuRef = useTemplateRef<HTMLDivElement>("projectMenu");

const selectedProject = () => props.projects?.find((p) => p.id === selectedProjectId.value);

function reset() {
  title.value = "";
  notes.value = "";
  scheduledDate.value = props.defaultScheduledDate ?? null;
  selectedProjectId.value = props.defaultProjectId ?? null;
  billable.value = Boolean(selectedProject()?.billable);
  priceInput.value = "";
  showProjectMenu.value = false;
}

function close() {
  emit("update:open", false);
  reset();
}

function save() {
  const trimmed = title.value.trim();
  if (!trimmed) {
    close();
    return;
  }
  const parsedPrice = priceInput.value.trim() === "" ? null : Number(priceInput.value);
  emit("save", {
    title: trimmed,
    notes: notes.value || null,
    scheduledDate: scheduledDate.value,
    projectId: selectedProjectId.value,
    billable: billable.value,
    price: Number.isFinite(parsedPrice) ? (parsedPrice as number) : null,
  });
  close();
}

function onTitleKeyDown(e: KeyboardEvent) {
  if (e.key === "Enter") {
    e.preventDefault();
    save();
  }
  if (e.key === "Escape") {
    e.preventDefault();
    close();
  }
}

function onNotesKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    close();
  }
}

watch(
  () => props.open,
  (val) => {
    if (val) {
      reset();
      nextTick(() => titleRef.value?.focus());
    }
  },
);

// Outside-click для project menu. Раньше использовался nested watch({ once: true })
// для cleanup'а — он ломался при последовательных open/close (новый handler
// добавлялся раньше чем предыдущий cleanup'ался) и не отписывался на unmount.
// Идиома `watch open ⇒ add/remove` + `onBeforeUnmount → remove` — symmetric и
// безопасна (mirror ContextMenu.vue).
function onProjectMenuOutsideClick(e: MouseEvent) {
  if (projectMenuRef.value && !projectMenuRef.value.contains(e.target as Node)) {
    showProjectMenu.value = false;
  }
}

watch(showProjectMenu, (val) => {
  if (val) {
    document.addEventListener("mousedown", onProjectMenuOutsideClick);
  } else {
    document.removeEventListener("mousedown", onProjectMenuOutsideClick);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("mousedown", onProjectMenuOutsideClick);
});
</script>

<template>
  <div v-if="open" class="absolute inset-0 z-40 flex items-start justify-center pt-24">
    <!-- Backdrop -->
    <div
      class="absolute inset-0 bg-[color-mix(in_srgb,var(--background)_40%,transparent)] backdrop-blur-sm"
      @click="close"
    />

    <!-- Panel -->
    <div
      class="relative z-10 w-full max-w-screen-sm overflow-visible rounded-xl border border-(--border) bg-(--color-shape-highlight-light-solid)"
    >
      <div class="flex flex-col gap-4 px-4 pt-4">
        <input
          ref="titleInput"
          type="text"
          placeholder="Новая задача"
          :value="title"
          class="w-full border-0 bg-transparent text-sm font-semibold text-(--foreground) outline-none placeholder:text-(--muted-foreground)"
          @input="title = ($event.target as HTMLInputElement).value"
          @keydown="onTitleKeyDown"
        />

        <textarea
          placeholder="Заметки"
          :value="notes"
          :rows="2"
          class="w-full resize-none border-0 bg-transparent text-xs text-(--muted-foreground) outline-none placeholder:text-[color-mix(in_srgb,var(--muted-foreground)_60%,transparent)]"
          @input="notes = ($event.target as HTMLTextAreaElement).value"
          @keydown="onNotesKeyDown"
        />
      </div>

      <div class="mt-4 h-px bg-(--border)" />

      <div class="px-4 py-2">
        <div class="flex flex-wrap items-center gap-2">
          <DateChip
            :value="scheduledDate"
            placeholder="Без даты"
            @update:value="scheduledDate = $event"
          />

          <button
            type="button"
            :class="[
              'inline-flex h-8 items-center gap-2 rounded-full border-0 px-4 text-xs transition-colors',
              billable
                ? 'bg-[color-mix(in_srgb,var(--status-success)_15%,transparent)] text-(--status-success) hover:bg-[color-mix(in_srgb,var(--status-success)_22%,transparent)]'
                : 'bg-(--secondary) text-(--muted-foreground) hover:bg-(--surface)',
            ]"
            :title="billable ? 'Оплачиваемая задача' : 'Сделать оплачиваемой'"
            @click="billable = !billable"
          >
            <DollarSign :size="16" />
            <span>Оплачиваемая</span>
          </button>

          <label
            v-if="billable"
            class="inline-flex h-8 cursor-text items-center gap-2 rounded-full border-0 bg-(--secondary) px-4 text-xs text-(--muted-foreground)"
          >
            <input
              type="number"
              inputmode="decimal"
              min="0"
              step="0.01"
              placeholder="Цена"
              :value="priceInput"
              class="w-16 border-0 bg-transparent text-xs text-(--foreground) outline-none placeholder:text-[color-mix(in_srgb,var(--muted-foreground)_60%,transparent)]"
              @input="priceInput = ($event.target as HTMLInputElement).value"
            />
          </label>

          <div class="flex-1" />

          <div ref="projectMenu" class="relative">
            <button
              type="button"
              :class="[
                'inline-flex h-8 items-center gap-2 rounded-full border-0 px-4 text-xs transition-colors',
                selectedProject()
                  ? 'bg-[color-mix(in_srgb,var(--accent)_15%,transparent)] text-(--accent) hover:bg-[color-mix(in_srgb,var(--accent)_22%,transparent)]'
                  : 'bg-(--secondary) text-(--muted-foreground) hover:bg-(--surface)',
              ]"
              @click="showProjectMenu = !showProjectMenu"
            >
              <Folder :size="16" />
              <span>{{ selectedProject()?.title ?? "Входящие" }}</span>
            </button>

            <div
              v-if="showProjectMenu"
              class="absolute right-0 top-full z-20 mt-2 max-h-64 min-w-48 overflow-y-auto rounded-lg border border-(--border) bg-(--color-shape-highlight-light-solid) py-2"
            >
              <button
                type="button"
                :class="[
                  'block w-full border-0 bg-transparent px-4 py-2 text-left text-xs text-(--foreground) hover:bg-(--surface)',
                  selectedProjectId === null ? 'text-(--accent)' : '',
                ]"
                @click="
                  selectedProjectId = null;
                  billable = false;
                  showProjectMenu = false;
                "
              >
                Входящие
              </button>
              <button
                v-for="project in projects ?? []"
                :key="project.id"
                type="button"
                :class="[
                  'block w-full border-0 bg-transparent px-4 py-2 text-left text-xs text-(--foreground) hover:bg-(--surface)',
                  selectedProjectId === project.id ? 'text-(--accent)' : '',
                ]"
                @click="
                  selectedProjectId = project.id;
                  billable = Boolean(project.billable);
                  showProjectMenu = false;
                "
              >
                {{ project.title }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <button
        type="button"
        class="absolute right-2 top-2 inline-flex size-8 items-center justify-center rounded-md border-0 bg-transparent p-2 text-(--muted-foreground) transition-colors hover:bg-(--surface) hover:text-(--foreground)"
        @click="close"
      >
        <X :size="16" />
      </button>
    </div>
  </div>
</template>
