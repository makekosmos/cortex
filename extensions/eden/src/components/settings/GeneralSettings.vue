<template>
  <div class="settings-tab">
    <h1 class="settings-tab-title">Общие</h1>
    <p class="settings-tab-subtitle">Основные настройки пространства</p>

    <div class="settings-sections">
      <section class="settings-section">
        <div class="settings-section-header">
          <h2>Хранилище</h2>
        </div>
        <div class="settings-section-body">
          <div class="settings-row">
            <div class="settings-row-left">
              <div class="settings-row-title">Источник данных</div>
              <div class="settings-row-desc">{{ vaultPath || "ARK (через Kepler)" }}</div>
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup vapor lang="ts">
defineProps<{
  vaultPath: string;
}>();

defineEmits<{
  selectVault: [];
}>();
</script>
