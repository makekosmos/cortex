<template>
  <div class="vault-sidebar">
    <aside
      class="vault-browser sidebarPage pageVault pageVaultSingle"
      data-testid="vault-switcher-menu"
    >
      <div class="head">
        <div class="side left">
          <div class="name">Хранилища</div>
        </div>
        <div class="side right">
          <button
            class="sidebar-head-icon withBackground"
            title="Добавить хранилище"
            type="button"
            @click="emit('openVaultPicker')"
          >
            <span aria-hidden="true" class="anytype-icon plus" />
          </button>
          <button
            class="sidebar-head-icon withBackground"
            title="Скрыть хранилища"
            type="button"
            @click="emit('toggleCollapsed')"
          >
            <span aria-hidden="true" class="anytype-icon toggleVault" />
          </button>
        </div>
      </div>

      <div class="filterWrapper">
        <input
          class="vault-browser-search"
          type="text"
          placeholder="Поиск"
          :value="filterQuery"
          @input="filterQuery = ($event.target as HTMLInputElement).value"
        />
      </div>

      <div class="body">
        <div class="scrollArea vault-browser-list">
          <button
            v-for="item in filteredVaults"
            :key="item"
            class="item vault-browser-item"
            :class="{ active: item === vaultPath }"
            :data-testid="`vault-item-${getVaultLabel(item)}`"
            type="button"
            @click="emit('selectVault', item)"
          >
            <span class="iconWrap">
              <span class="vault-avatar">{{ getVaultInitial(item) }}</span>
            </span>
            <span class="info">
              <span class="nameWrapper">
                <span class="name">{{ getVaultLabel(item) }}</span>
              </span>
              <span class="messageWrapper">
                <span class="label lastMessage">Локальная папка</span>
              </span>
            </span>
          </button>
          <div v-if="filteredVaults.length === 0" class="vault-browser-empty">
            Ничего не найдено
          </div>
        </div>
      </div>

      <div class="bottom">
        <div class="grad" />
        <div class="sides">
          <div class="side left">
            <button
              class="appSettings"
              type="button"
              @click="vaultPath ? emit('selectVault', vaultPath) : undefined"
            >
              <span class="iconWrap">
                <span class="vault-avatar">{{ vaultPath ? getVaultInitial(vaultPath) : "E" }}</span>
              </span>
              <span class="name">{{ vaultPath ? getVaultLabel(vaultPath) : "Eden" }}</span>
            </button>
          </div>
        </div>
      </div>
    </aside>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";

const props = defineProps<{
  vaultPath: string | null;
  recentVaultPaths: string[];
  collapsed: boolean;
}>();

const emit = defineEmits<{
  toggleCollapsed: [];
  selectVault: [vaultPath: string];
  openVaultPicker: [];
}>();

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
