<script setup lang="ts">
import { shallowRef, watch, nextTick, useTemplateRef } from "vue";
import { DollarSign, Folder, X } from "lucide-vue-next";
import DateChip from "./DateChip.vue";

export interface QuickEntryProject {
  id: string;
  title: string;
  billable?: boolean;
}

export interface QuickEntrySavePayload {
  title: string;
  notes: string | null;
  scheduledDate: string | null;
  projectId: string | null;
  billable: boolean;
  price: number | null;
}

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

const selectedProject = () =>
  props.projects?.find((p) => p.id === selectedProjectId.value);

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
  if (e.key === "Enter") { e.preventDefault(); save(); }
  if (e.key === "Escape") { e.preventDefault(); close(); }
}

function onNotesKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape") { e.preventDefault(); close(); }
}

watch(() => props.open, (val) => {
  if (val) {
    reset();
    nextTick(() => titleRef.value?.focus());
  }
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
    class="absolute inset-0 z-40 flex items-start justify-center pt-[15vh]"
  >
    <!-- Backdrop — only covers content area, not titlebar/sidebar -->
    <div class="absolute inset-0 bg-black/40 backdrop-blur-sm" @click="close" />

    <!-- Panel -->
    <div
      class="relative z-10 w-full max-w-(--bringhurst-wide) overflow-visible rounded-xl border border-(--border) shadow-2xl"
      style="background: var(--color-shape-highlight-light-solid)"
    >
      <div class="flex flex-col gap-3 px-4 pt-3">
        <input
          ref="titleInput"
          type="text"
          placeholder="Новая задача"
          :value="title"
          class="w-full bg-transparent text-sm font-semibold text-(--foreground) placeholder:text-(--muted-foreground) outline-none"
          @input="title = ($event.target as HTMLInputElement).value"
          @keydown="onTitleKeyDown"
        />

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
          <DateChip
            :value="scheduledDate"
            placeholder="Без даты"
            @update:value="scheduledDate = $event"
          />

          <!-- Billable toggle -->
          <button
            type="button"
            :class="[
              'flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs transition-colors',
              billable
                ? 'bg-emerald-500/15 text-emerald-500'
                : 'bg-(--secondary) text-(--muted-foreground) hover:bg-(--surface)',
            ]"
            :title="billable ? 'Оплачиваемая задача' : 'Сделать оплачиваемой'"
            @click="billable = !billable"
          >
            <DollarSign :size="12" />
            <span>Оплачиваемая</span>
          </button>

          <!-- Price input (only when billable) -->
          <label
            v-if="billable"
            class="flex items-center gap-1.5 rounded-full bg-(--secondary) px-3 py-1.5 text-xs"
          >
            <input
              type="number"
              inputmode="decimal"
              min="0"
              step="0.01"
              placeholder="Цена"
              :value="priceInput"
              class="w-20 bg-transparent text-xs outline-none placeholder:text-(--muted-foreground)/60"
              @input="priceInput = ($event.target as HTMLInputElement).value"
            />
          </label>

          <div class="flex-1" />

          <!-- Project picker -->
          <div ref="projectMenu" class="relative">
            <button
              type="button"
              :class="[
                'flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs transition-colors',
                selectedProject()
                  ? 'bg-(--accent)/15 text-(--accent)'
                  : 'bg-(--secondary) text-(--muted-foreground) hover:bg-(--surface)',
              ]"
              @click="showProjectMenu = !showProjectMenu"
            >
              <Folder :size="12" />
              <span>{{ selectedProject()?.title ?? "Входящие" }}</span>
            </button>

            <div
              v-if="showProjectMenu"
              class="absolute right-0 top-full z-20 mt-1 min-w-45 max-h-64 overflow-y-auto rounded-lg border border-(--border) py-1 shadow-lg"
              style="background: var(--color-shape-highlight-light-solid)"
            >
              <button
                type="button"
                :class="[
                  'w-full px-3 py-1.5 text-left text-xs hover:bg-(--surface)',
                  selectedProjectId === null ? 'text-(--accent)' : 'text-(--foreground)',
                ]"
                @click="selectedProjectId = null; billable = false; showProjectMenu = false"
              >
                Входящие
              </button>
              <button
                v-for="project in (projects ?? [])"
                :key="project.id"
                type="button"
                :class="[
                  'w-full px-3 py-1.5 text-left text-xs hover:bg-(--surface)',
                  selectedProjectId === project.id ? 'text-(--accent)' : 'text-(--foreground)',
                ]"
                @click="selectedProjectId = project.id; billable = Boolean(project.billable); showProjectMenu = false"
              >
                {{ project.title }}
              </button>
            </div>
          </div>
        </div>
      </div>

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
