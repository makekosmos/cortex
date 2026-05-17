<template>
  <div class="settings-tab">
    <h1 class="settings-tab-title">Облачное хранилище</h1>
    <p class="settings-tab-subtitle">
      Вы можете хранить ваши файлы на нашем зашифрованном узле резервного копирования. Как только вы
      достигнете лимита, файлы перестанут синхронизироваться и будут храниться только локально.
    </p>

    <div class="settings-sections">
      <div class="storage-usage-wrapper">
        <div class="storage-progress-bar">
          <div class="storage-bar-track">
            <div
              class="storage-bar-segment current"
              :style="{ width: `${Math.max(currentPercent, 0.3)}%` }"
            />
            <div
              class="storage-bar-segment other"
              :style="{ width: `${Math.max(otherPercent, 0.2)}%` }"
            />
            <div class="storage-bar-segment empty" :style="{ width: `${freePercent}%` }" />
          </div>
        </div>
        <div class="storage-usage-info">
          <div class="storage-usage-total">
            <span class="storage-usage-used">{{ formatBytes(totalUsed) }} </span>
            {{ formatBytes(TOTAL_CAPACITY) }} использовано
          </div>
          <div class="storage-usage-legend">
            <div class="storage-legend-entry">
              <span class="storage-legend-marker current" />{{ spaceName }}
            </div>
            <div class="storage-legend-entry">
              <span class="storage-legend-marker other" />Другие пространства
            </div>
            <div class="storage-legend-entry">
              <span class="storage-legend-marker free" />Свободное место
            </div>
          </div>
        </div>
      </div>

      <div class="storage-file-manager">
        <div class="storage-tabs">
          <div class="storage-tab-item active">Синхронизировано</div>
        </div>
        <div class="storage-controls">
          <div class="storage-controls-left">
            <button class="storage-checkbox-btn" type="button" @click="toggleSelectAll">
              <span class="storage-checkbox-icon" :class="{ checked: allSelected }" />
              <span>Выбрать всё</span>
            </button>
          </div>
          <div class="storage-controls-right">
            <button class="storage-icon-btn" type="button" @click="filterOpen = !filterOpen">
              <svg
                width="20"
                height="20"
                viewBox="0 0 20 20"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
              >
                <circle cx="8.5" cy="8.5" r="5.5" />
                <path d="M13 13L17 17" />
              </svg>
            </button>
            <div class="storage-filter-wrap" :class="{ active: filterOpen }">
              <input
                class="storage-filter-input"
                type="text"
                placeholder="Поиск..."
                v-model="searchQuery"
              />
            </div>
          </div>
        </div>
        <div class="storage-file-list">
          <div v-for="file in filteredFiles" :key="file.id" class="storage-file-row">
            <button
              class="storage-checkbox-icon"
              :class="{ checked: selectedIds.has(file.id) }"
              type="button"
              @click="toggleFile(file.id)"
            />
            <div class="storage-file-click-area">
              <div
                class="storage-file-icon"
                :class="file.type === 'image' ? 'is-image' : 'is-file'"
              >
                <svg
                  v-if="file.type === 'image'"
                  width="18"
                  height="18"
                  viewBox="0 0 18 18"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.2"
                >
                  <rect x="2" y="2" width="14" height="14" rx="2" />
                  <circle cx="6.5" cy="6.5" r="1.5" />
                  <path d="M2 13L6 9L10 13" />
                  <path d="M9 11L12 8L16 12" />
                </svg>
                <svg
                  v-else
                  width="18"
                  height="18"
                  viewBox="0 0 18 18"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.2"
                >
                  <path d="M4 2H11L14 5V16H4V2Z" />
                  <path d="M11 2V5H14" />
                </svg>
              </div>
              <div class="storage-file-info">
                <div class="storage-file-name">
                  <span>{{ file.name }}</span>
                </div>
                <div class="storage-file-size">{{ formatBytes(file.size) }}</div>
              </div>
            </div>
          </div>
          <div v-if="filteredFiles.length === 0" class="storage-file-empty">Ничего не найдено</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";

const props = defineProps<{ vaultPath: string }>();

interface StorageFile {
  id: string;
  name: string;
  size: number;
  type: "image" | "file";
}

const TOTAL_CAPACITY = 20 * 1024 * 1024 * 1024;
const MOCK_FILES: StorageFile[] = [
  { id: "1", name: "1000044850.png", size: 2.09 * 1024 * 1024, type: "image" },
  { id: "2", name: "image_1773323557702_0.png", size: 802.23 * 1024, type: "image" },
  { id: "3", name: "1000045388.webp", size: 554.3 * 1024, type: "image" },
  { id: "4", name: "yd5mi4z4e20hytl9tmcporvs2zugx9y7.jpeg", size: 402.35 * 1024, type: "image" },
  { id: "5", name: "1000045401.jpg", size: 362.43 * 1024, type: "image" },
  { id: "6", name: "deniel-kiz.jpg", size: 90.19 * 1024, type: "image" },
  { id: "7", name: "00044830.jpg", size: 31.46 * 1024, type: "image" },
  { id: "8", name: "00044830.jpg", size: 19.54 * 1024, type: "image" },
  { id: "9", name: "lab721_com_br_icon.ico", size: 15.04 * 1024, type: "file" },
];

const searchQuery = ref("");
const selectedIds = ref(new Set<string>());
const filterOpen = ref(false);

const currentSpaceBytes = 60.03 * 1024 * 1024;
const otherSpacesBytes = 5.1 * 1024 * 1024;
const totalUsed = currentSpaceBytes + otherSpacesBytes;
const currentPercent = (currentSpaceBytes / TOTAL_CAPACITY) * 100;
const otherPercent = (otherSpacesBytes / TOTAL_CAPACITY) * 100;
const freePercent = 100 - currentPercent - otherPercent;

const spaceName = computed(() => props.vaultPath.split(/[/\\]/).pop() || "Eden");
const filteredFiles = computed(() =>
  MOCK_FILES.filter((f) => f.name.toLowerCase().includes(searchQuery.value.toLowerCase())),
);
const allSelected = computed(
  () =>
    filteredFiles.value.length > 0 && filteredFiles.value.every((f) => selectedIds.value.has(f.id)),
);

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(2)} ${units[i]}`;
}

function toggleSelectAll() {
  if (allSelected.value) {
    selectedIds.value = new Set();
  } else {
    selectedIds.value = new Set(filteredFiles.value.map((f) => f.id));
  }
}

function toggleFile(id: string) {
  const next = new Set(selectedIds.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  selectedIds.value = next;
}
</script>
