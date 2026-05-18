<script setup lang="ts">
import { shallowRef, watch, nextTick, useTemplateRef, onBeforeUnmount } from "vue";
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
  <div v-if="open" class="qep-root">
    <!-- Backdrop -->
    <div class="qep-backdrop" @click="close" />

    <!-- Panel -->
    <div class="qep-panel">
      <div class="qep-content">
        <input
          ref="titleInput"
          type="text"
          placeholder="Новая задача"
          :value="title"
          class="qep-title"
          @input="title = ($event.target as HTMLInputElement).value"
          @keydown="onTitleKeyDown"
        />

        <textarea
          placeholder="Заметки"
          :value="notes"
          :rows="2"
          class="qep-notes"
          @input="notes = ($event.target as HTMLTextAreaElement).value"
          @keydown="onNotesKeyDown"
        />
      </div>

      <div class="qep-divider" />

      <div class="qep-meta">
        <div class="qep-meta-row">
          <DateChip
            :value="scheduledDate"
            placeholder="Без даты"
            @update:value="scheduledDate = $event"
          />

          <button
            type="button"
            class="qep-chip"
            :class="{ 'qep-chip--billable': billable }"
            :title="billable ? 'Оплачиваемая задача' : 'Сделать оплачиваемой'"
            @click="billable = !billable"
          >
            <DollarSign :size="12" />
            <span>Оплачиваемая</span>
          </button>

          <label v-if="billable" class="qep-chip qep-chip--input">
            <input
              type="number"
              inputmode="decimal"
              min="0"
              step="0.01"
              placeholder="Цена"
              :value="priceInput"
              class="qep-price-input"
              @input="priceInput = ($event.target as HTMLInputElement).value"
            />
          </label>

          <div class="qep-spacer" />

          <div ref="projectMenu" class="qep-project-wrap">
            <button
              type="button"
              class="qep-chip"
              :class="{ 'qep-chip--active': selectedProject() }"
              @click="showProjectMenu = !showProjectMenu"
            >
              <Folder :size="12" />
              <span>{{ selectedProject()?.title ?? "Входящие" }}</span>
            </button>

            <div v-if="showProjectMenu" class="qep-menu">
              <button
                type="button"
                class="qep-menu-item"
                :class="{ 'qep-menu-item--selected': selectedProjectId === null }"
                @click="selectedProjectId = null; billable = false; showProjectMenu = false"
              >
                Входящие
              </button>
              <button
                v-for="project in (projects ?? [])"
                :key="project.id"
                type="button"
                class="qep-menu-item"
                :class="{ 'qep-menu-item--selected': selectedProjectId === project.id }"
                @click="selectedProjectId = project.id; billable = Boolean(project.billable); showProjectMenu = false"
              >
                {{ project.title }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <button type="button" class="qep-close" @click="close">
        <X :size="14" />
      </button>
    </div>
  </div>
</template>

<style scoped>
/* QuickEntryPanel — modal for task creation. Использует @kosmos/visuals
   tokens, никакого Tailwind. Временный handcrafted-кандидат — стиль будет
   дополирован, см. STORYBOOK handcrafted tag. */

.qep-root {
  position: absolute;
  inset: 0;
  z-index: 40;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 15vh;
}

.qep-backdrop {
  position: absolute;
  inset: 0;
  background: color-mix(in srgb, #000 40%, transparent);
  backdrop-filter: blur(4px);
}

.qep-panel {
  position: relative;
  z-index: 10;
  width: 100%;
  max-width: var(--bringhurst-wide);
  background: var(--color-shape-highlight-light-solid, var(--background));
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: 0 16px 48px color-mix(in srgb, #000 40%, transparent);
  overflow: visible;
}

.qep-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px 16px 0;
}

.qep-title,
.qep-notes {
  width: 100%;
  background: transparent;
  border: none;
  outline: none;
  font-family: inherit;
  color: var(--foreground);
}

.qep-title {
  font-size: 14px;
  font-weight: 600;
}
.qep-title::placeholder {
  color: var(--muted-foreground);
}

.qep-notes {
  font-size: 12px;
  color: var(--muted-foreground);
  resize: none;
}
.qep-notes::placeholder {
  color: color-mix(in srgb, var(--muted-foreground) 60%, transparent);
}

.qep-divider {
  height: 1px;
  background: var(--border);
  margin-top: 12px;
}

.qep-meta {
  padding: 8px 16px;
}

.qep-meta-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.qep-spacer {
  flex: 1;
}

.qep-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 12px;
  background: var(--secondary);
  color: var(--muted-foreground);
  border: none;
  border-radius: 999px;
  font-family: inherit;
  font-size: 12px;
  cursor: pointer;
  transition: background-color 120ms, color 120ms;
}
.qep-chip:hover {
  background: var(--surface);
}
.qep-chip--billable {
  background: color-mix(in srgb, var(--status-success) 15%, transparent);
  color: var(--status-success);
}
.qep-chip--billable:hover {
  background: color-mix(in srgb, var(--status-success) 22%, transparent);
}
.qep-chip--active {
  background: color-mix(in srgb, var(--accent) 15%, transparent);
  color: var(--accent);
}
.qep-chip--active:hover {
  background: color-mix(in srgb, var(--accent) 22%, transparent);
}
.qep-chip--input {
  padding: 0 10px;
  cursor: text;
}

.qep-price-input {
  width: 70px;
  background: transparent;
  border: none;
  outline: none;
  font-family: inherit;
  font-size: 12px;
  color: var(--foreground);
}
.qep-price-input::placeholder {
  color: color-mix(in srgb, var(--muted-foreground) 60%, transparent);
}

.qep-project-wrap {
  position: relative;
}

.qep-menu {
  position: absolute;
  right: 0;
  top: 100%;
  z-index: 20;
  margin-top: 4px;
  min-width: 180px;
  max-height: 256px;
  overflow-y: auto;
  padding: 4px 0;
  background: var(--color-shape-highlight-light-solid, var(--background));
  border: 1px solid var(--border);
  border-radius: 8px;
  box-shadow: 0 8px 24px color-mix(in srgb, #000 28%, transparent);
}

.qep-menu-item {
  display: block;
  width: 100%;
  padding: 6px 12px;
  text-align: left;
  background: transparent;
  border: none;
  font-family: inherit;
  font-size: 12px;
  color: var(--foreground);
  cursor: pointer;
}
.qep-menu-item:hover {
  background: var(--surface);
}
.qep-menu-item--selected {
  color: var(--accent);
}

.qep-close {
  position: absolute;
  right: 10px;
  top: 10px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 4px;
  background: transparent;
  border: none;
  color: var(--muted-foreground);
  border-radius: 6px;
  cursor: pointer;
  transition: background 100ms, color 100ms;
}
.qep-close:hover {
  background: var(--surface);
  color: var(--foreground);
}
</style>
