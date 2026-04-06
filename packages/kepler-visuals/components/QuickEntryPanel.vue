<script setup lang="ts">
import { shallowRef, watch, nextTick, useTemplateRef } from "vue";
import { Calendar, Folder, Moon, Star, X } from "lucide-vue-next";

export interface QuickEntryProject {
  id: string;
  title: string;
}

export interface QuickEntrySavePayload {
  title: string;
  notes: string | null;
  isToday: boolean;
  isEvening: boolean;
  scheduledDate: string | null;
  projectId: string | null;
}

const props = defineProps<{
  open: boolean;
  projects?: QuickEntryProject[];
}>();

const emit = defineEmits<{
  "update:open": [boolean];
  save: [payload: QuickEntrySavePayload];
}>();

const title = shallowRef("");
const notes = shallowRef("");
const isToday = shallowRef(false);
const isEvening = shallowRef(false);
const scheduledDate = shallowRef("");
const selectedProjectId = shallowRef<string | null>(null);
const showProjectMenu = shallowRef(false);

const titleRef = useTemplateRef<HTMLInputElement>("titleInput");
const projectMenuRef = useTemplateRef<HTMLDivElement>("projectMenu");

const selectedProject = () =>
  props.projects?.find((p) => p.id === selectedProjectId.value);

function reset() {
  title.value = "";
  notes.value = "";
  isToday.value = false;
  isEvening.value = false;
  scheduledDate.value = "";
  selectedProjectId.value = null;
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
  emit("save", {
    title: trimmed,
    notes: notes.value || null,
    isToday: isToday.value,
    isEvening: isEvening.value,
    scheduledDate: scheduledDate.value || null,
    projectId: selectedProjectId.value,
  });
  close();
}

function onTitleKeyDown(e: KeyboardEvent) {
  if (e.key === "Enter") { e.preventDefault(); save(); }
  if (e.key === "Escape") { e.preventDefault(); close(); }
}

function onNotesKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape") { e.preventDefault(); close(); }
}

function toggleToday() {
  const prev = isToday.value;
  isToday.value = !prev;
  if (!prev) isEvening.value = false;
}

function toggleEvening() {
  const prev = isEvening.value;
  isEvening.value = !prev;
  if (!prev) isToday.value = true;
}

watch(() => props.open, (val) => {
  if (val) nextTick(() => titleRef.value?.focus());
});

watch(showProjectMenu, (val) => {
  if (!val) return;
  const handler = (e: MouseEvent) => {
    if (projectMenuRef.value && !projectMenuRef.value.contains(e.target as Node)) {
      showProjectMenu.value = false;
    }
  };
  document.addEventListener("mousedown", handler);
  watch(showProjectMenu, () => document.removeEventListener("mousedown", handler), { once: true });
});
</script>

<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]"
  >
    <!-- Backdrop -->
    <div class="absolute inset-0 bg-black/40 backdrop-blur-sm" @click="close" />

    <!-- Panel -->
    <div
      class="relative z-10 w-full max-w-(--bringhurst-wide) overflow-hidden rounded-xl border border-(--border) shadow-2xl"
      style="background: var(--color-shape-highlight-light-solid)"
    >
      <div class="flex flex-col gap-3 px-4 pt-3">
        <!-- Title -->
        <input
          ref="titleInput"
          type="text"
          placeholder="Новая задача"
          :value="title"
          class="w-full bg-transparent text-sm font-semibold text-(--foreground) placeholder:text-(--muted-foreground) outline-none"
          @input="title = ($event.target as HTMLInputElement).value"
          @keydown="onTitleKeyDown"
        />

        <!-- Notes -->
        <textarea
          placeholder="Заметки"
          :value="notes"
          :rows="2"
          class="w-full resize-none bg-transparent text-xs text-(--muted-foreground) placeholder:text-(--muted-foreground)/60 outline-none"
          @input="notes = ($event.target as HTMLTextAreaElement).value"
          @keydown="onNotesKeyDown"
        />
      </div>

      <div class="h-px bg-(--border)" />

      <!-- Metadata row -->
      <div class="px-4 py-2">
        <div class="flex flex-wrap items-center gap-2">
          <!-- Schedule date -->
          <label
            class="flex cursor-pointer items-center gap-1.5 rounded-full bg-(--secondary) px-3 py-1.5 text-xs transition-colors hover:bg-(--surface)"
          >
            <Calendar
              :size="12"
              :class="scheduledDate ? 'text-blue-500' : 'text-(--muted-foreground)'"
            />
            <input
              type="date"
              :value="scheduledDate"
              class="w-25 cursor-pointer bg-transparent text-xs"
              @input="scheduledDate = ($event.target as HTMLInputElement).value"
            />
          </label>

          <!-- Today -->
          <button
            type="button"
            :class="[
              'flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs transition-colors',
              isToday
                ? 'bg-yellow-500/15 text-yellow-500'
                : 'bg-(--secondary) text-(--muted-foreground) hover:bg-(--surface)',
            ]"
            @click="toggleToday"
          >
            <Star :size="12" />
            <span>Сегодня</span>
          </button>

          <!-- Evening -->
          <button
            type="button"
            :class="[
              'flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs transition-colors',
              isEvening
                ? 'bg-indigo-500/15 text-indigo-400'
                : 'bg-(--secondary) text-(--muted-foreground) hover:bg-(--surface)',
            ]"
            @click="toggleEvening"
          >
            <Moon :size="12" />
            <span>Вечер</span>
          </button>

          <div class="flex-1" />

          <!-- Project picker -->
          <div v-if="projects?.length" ref="projectMenu" class="relative">
            <button
              type="button"
              :class="[
                'flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs transition-colors',
                selectedProject()
                  ? 'bg-blue-500/15 text-blue-500'
                  : 'bg-(--secondary) text-(--muted-foreground) hover:bg-(--surface)',
              ]"
              @click="showProjectMenu = !showProjectMenu"
            >
              <Folder :size="12" />
              <span>{{ selectedProject()?.title ?? "Входящие" }}</span>
            </button>

            <div
              v-if="showProjectMenu"
              class="absolute right-0 top-full z-20 mt-1 min-w-45 rounded-lg border border-(--border) py-1 shadow-lg"
              style="background: var(--color-shape-highlight-light-solid)"
            >
              <button
                type="button"
                class="w-full px-3 py-1.5 text-left text-xs text-(--foreground) hover:bg-(--surface)"
                @click="selectedProjectId = null; showProjectMenu = false"
              >
                Входящие
              </button>
              <button
                v-for="project in projects"
                :key="project.id"
                type="button"
                class="w-full px-3 py-1.5 text-left text-xs text-(--foreground) hover:bg-(--surface)"
                @click="selectedProjectId = project.id; showProjectMenu = false"
              >
                {{ project.title }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- Close button -->
      <button
        type="button"
        class="absolute right-2.5 top-2.5 rounded-md p-1 text-(--muted-foreground) hover:bg-(--surface) hover:text-(--foreground)"
        @click="close"
      >
        <X :size="14" />
      </button>
    </div>
  </div>
</template>
