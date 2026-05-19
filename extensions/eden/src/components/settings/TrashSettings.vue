<template>
  <div class="settings-tab">
    <div class="settings-tab-header-row">
      <div>
        <h1 class="settings-tab-title">Корзина</h1>
        <p class="settings-tab-subtitle">
          Удалённые заметки хранятся 30 дней, затем удаляются навсегда.
        </p>
      </div>
      <button
        v-if="trashEntries.length > 0"
        class="settings-btn-danger"
        type="button"
        @click="handleEmptyTrash"
      >
        Очистить корзину
      </button>
    </div>

    <div class="settings-sections">
      <div v-if="loading" class="settings-empty">Загрузка...</div>
      <div v-else-if="trashEntries.length === 0" class="settings-empty">Корзина пуста</div>
      <div v-else class="trash-list">
        <div v-for="entry in trashEntries" :key="entry.id" class="trash-item">
          <div class="trash-item-info">
            <img
              class="trash-item-icon"
              :src="trashItemIcon"
              alt=""
              width="18"
              height="18"
              draggable="false"
            />
            <div class="trash-item-text">
              <span class="trash-item-title">{{ getEntryDisplayTitle(entry.title, entry.header_props_json) }}</span>
              <span class="trash-item-meta">
                Удалено {{ formatTimeAgo(entry.deleted_at!) }} · осталось
                {{ daysRemaining(entry.deleted_at!) }} дн.
              </span>
            </div>
          </div>
          <div class="trash-item-actions">
            <button class="settings-btn-secondary" type="button" @click="handleRestore(entry.id)">
              Восстановить
            </button>
            <button
              class="settings-btn-danger-sm"
              type="button"
              @click="handlePermanentDelete(entry.id)"
            >
              Удалить
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup vapor lang="ts">
import { ref, onMounted } from "vue";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { objectIconUri } from "@/lib/iconResolver";

const trashItemIcon = objectIconUri("page");

const emit = defineEmits<{ refreshData: [] }>();

const trashEntries = ref<Entry[]>([]);
const loading = ref(true);

function formatTimeAgo(deletedAt: number): string {
  const days = Math.floor((Date.now() - deletedAt) / (1000 * 60 * 60 * 24));
  if (days === 0) return "сегодня";
  if (days === 1) return "вчера";
  if (days < 7) return `${days} дн. назад`;
  return `${Math.floor(days / 7)} нед. назад`;
}

function daysRemaining(deletedAt: number): number {
  return Math.max(0, 30 - Math.floor((Date.now() - deletedAt) / (1000 * 60 * 60 * 24)));
}

async function loadTrash() {
  if (!window.api?.listTrashEntries) return;
  loading.value = true;
  trashEntries.value = await window.api.listTrashEntries();
  loading.value = false;
}

async function handleRestore(entryId: string) {
  await window.api.restoreEntry(entryId);
  await loadTrash();
  emit("refreshData");
}

async function handlePermanentDelete(entryId: string) {
  await window.api.permanentDeleteEntry(entryId);
  await loadTrash();
}

async function handleEmptyTrash() {
  for (const entry of trashEntries.value) {
    await window.api.permanentDeleteEntry(entry.id);
  }
  await loadTrash();
}

onMounted(() => {
  void loadTrash();
});
</script>
