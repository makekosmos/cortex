<template>
  <div class="vault-rail-shell">
    <aside class="vault-rail" data-testid="vault-rail">
      <div class="vault-rail-list">
        <button
          v-if="vaultPath"
          class="vault-rail-item is-active"
          :data-testid="`vault-item-${getVaultLabel(vaultPath)}`"
          :title="getVaultLabel(vaultPath)"
          type="button"
          @click="emit('selectVault', vaultPath)"
        >
          <span>{{ getVaultInitial(vaultPath) }}</span>
        </button>
      </div>
      <div class="vault-rail-actions">
        <button
          aria-label="Открыть список хранилищ"
          class="vault-rail-switcher"
          data-testid="vault-switcher-toggle"
          title="Хранилища"
          type="button"
          @click="isListOpen = !isListOpen"
        >
          ≡
        </button>
      </div>
    </aside>

    <aside v-if="isListOpen" class="vault-browser" data-testid="vault-switcher-menu">
      <div class="vault-browser-header">
        <strong>Хранилища</strong>
        <div class="vault-browser-actions">
          <button
            class="vault-browser-action-btn"
            title="Добавить хранилище"
            type="button"
            @click="emit('openVaultPicker')"
          >
            +
          </button>
          <button
            class="vault-browser-action-btn"
            title="Скрыть список"
            type="button"
            @click="isListOpen = false"
          >
            ×
          </button>
        </div>
      </div>
      <input
        class="vault-browser-search"
        type="text"
        placeholder="Поиск хранилищ"
        :value="filterQuery"
        @input="filterQuery = ($event.target as HTMLInputElement).value"
      />
      <div class="vault-browser-list">
        <button
          v-for="item in filteredVaults"
          :key="item"
          class="vault-browser-item"
          :class="{ 'is-active': item === vaultPath }"
          type="button"
          @click="emit('selectVault', item)"
        >
          <span class="vault-browser-item-avatar">{{ getVaultInitial(item) }}</span>
          <span class="vault-browser-item-name">{{ getVaultLabel(item) }}</span>
        </button>
        <div v-if="filteredVaults.length === 0" class="vault-browser-empty">Ничего не найдено</div>
      </div>
    </aside>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";

const props = defineProps<{
  vaultPath: string | null;
  recentVaultPaths: string[];
}>();

const emit = defineEmits<{
  selectVault: [vaultPath: string];
  openVaultPicker: [];
}>();

const isListOpen = ref(true);
const filterQuery = ref("");

function getVaultLabel(path: string) {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

function getVaultInitial(path: string) {
  const label = getVaultLabel(path).trim();
  return label ? label[0]!.toUpperCase() : "E";
}

const visibleVaults = computed(() => {
  if (!props.vaultPath) return [];
  const merged = [props.vaultPath, ...props.recentVaultPaths.filter((v) => v !== props.vaultPath)];
  return merged.slice(0, 12);
});

const filteredVaults = computed(() => {
  const q = filterQuery.value.trim().toLowerCase();
  if (!q) return visibleVaults.value;
  return visibleVaults.value.filter((v) => getVaultLabel(v).toLowerCase().includes(q));
});
</script>
