<script setup lang="ts">
import {
  shallowRef,
  useTemplateRef,
  onMounted,
  onUnmounted,
  nextTick,
  watch,
} from "vue";
import { Calendar, Folder, Moon, Star, X } from "lucide-vue-next";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { useQuickEntry } from "@/composables/useQuickEntry";

const { open, hide } = useQuickEntry();
const title = shallowRef("");
const notes = shallowRef("");
const isToday = shallowRef(false);
const isEvening = shallowRef(false);
const scheduledDate = shallowRef("");
const selectedProjectId = shallowRef<string | null>(null);
const showProjectMenu = shallowRef(false);

const titleRef = useTemplateRef<HTMLInputElement>("titleInput");
const projectMenuRef = useTemplateRef<HTMLDivElement>("projectMenu");

const store = useTodoStore();
const { projects } = storeToRefs(store);

const selectedProject = () =>
  projects.value.find((p) => p.id === selectedProjectId.value);

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
  hide();
  reset();
}

function save() {
  const trimmed = title.value.trim();
  if (!trimmed) {
    close();
    return;
  }

  store.addTodo({
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

// Cmd+N to toggle
onMounted(() => {
  const handler = (e: KeyboardEvent) => {
    if (e.metaKey && e.key === "n" && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      if (open.value) {
        close();
      } else {
        open.value = true;
      }
    }
  };
  window.addEventListener("keydown", handler);
  onUnmounted(() => window.removeEventListener("keydown", handler));
});

// Focus title when opened
watch(open, (val) => {
  if (val) {
    nextTick(() => titleRef.value?.focus());
  }
});

// Close project menu on outside click
watch(showProjectMenu, (val) => {
  if (!val) return;
  const handler = (e: MouseEvent) => {
    if (
      projectMenuRef.value &&
      !projectMenuRef.value.contains(e.target as Node)
    ) {
      showProjectMenu.value = false;
    }
  };
  document.addEventListener("mousedown", handler);
  // Use a watch cleanup to remove the listener
  const cleanup = () => document.removeEventListener("mousedown", handler);
  onUnmounted(cleanup);
  watch(
    showProjectMenu,
    (newVal) => {
      if (!newVal) cleanup();
    },
    { once: true },
  );
});
</script>

<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]"
  >
    <!-- Backdrop -->
    <div class="absolute inset-0 bg-black/40" @click="close" />

    <!-- Panel -->
    <div
      class="relative z-10 w-120 rounded-2xl border border-(--border) bg-(--popover) shadow-2xl"
    >
      <div class="flex flex-col gap-3 p-5">
        <!-- Title -->
        <input
          ref="titleInput"
          type="text"
          placeholder="Новая задача"
          :value="title"
          class="w-full text-lg font-semibold text-(--foreground) placeholder:text-(--muted-foreground)"
          @input="title = ($event.target as HTMLInputElement).value"
          @keydown="onTitleKeyDown"
        />

        <!-- Notes -->
        <textarea
          placeholder="Заметки"
          :value="notes"
          :rows="2"
          class="w-full resize-none text-sm text-(--muted-foreground) placeholder:text-(--muted-foreground)/60"
          @input="notes = ($event.target as HTMLTextAreaElement).value"
          @keydown="onNotesKeyDown"
        />

        <!-- Divider -->
        <div class="h-px bg-(--border) opacity-50" />

        <!-- Metadata row -->
        <div class="flex flex-wrap items-center gap-2">
          <!-- Schedule date -->
          <label
            class="flex cursor-pointer items-center gap-1.5 rounded-full bg-(--secondary) px-3 py-1.5 text-xs transition-colors hover:bg-(--accent)"
          >
            <Calendar
              :size="12"
              :class="
                scheduledDate ? 'text-blue-500' : 'text-(--muted-foreground)'
              "
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
                : 'bg-(--secondary) text-(--muted-foreground) hover:bg-(--accent)',
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
                : 'bg-(--secondary) text-(--muted-foreground) hover:bg-(--accent)',
            ]"
            @click="toggleEvening"
          >
            <Moon :size="12" />
            <span>Вечер</span>
          </button>

          <div class="flex-1" />

          <!-- Project picker -->
          <div ref="projectMenu" class="relative">
            <button
              type="button"
              :class="[
                'flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs transition-colors',
                selectedProject()
                  ? 'bg-blue-500/15 text-blue-500'
                  : 'bg-(--secondary) text-(--muted-foreground) hover:bg-(--accent)',
              ]"
              @click="showProjectMenu = !showProjectMenu"
            >
              <Folder :size="12" />
              <span>{{ selectedProject()?.title ?? "Входящие" }}</span>
            </button>

            <div
              v-if="showProjectMenu"
              class="absolute right-0 top-full z-20 mt-1 min-w-45 rounded-lg border border-(--border) bg-(--popover) py-1 shadow-lg"
            >
              <button
                type="button"
                class="w-full px-3 py-1.5 text-left text-xs text-(--foreground) hover:bg-(--accent)"
                @click="
                  selectedProjectId = null;
                  showProjectMenu = false;
                "
              >
                Входящие
              </button>
              <button
                v-for="project in projects"
                :key="project.id"
                type="button"
                class="w-full px-3 py-1.5 text-left text-xs text-(--foreground) hover:bg-(--accent)"
                @click="
                  selectedProjectId = project.id;
                  showProjectMenu = false;
                "
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
        class="absolute right-3 top-3 rounded-md p-1 text-(--muted-foreground) hover:bg-(--accent) hover:text-(--foreground)"
        @click="close"
      >
        <X :size="14" />
      </button>
    </div>
  </div>
</template>
