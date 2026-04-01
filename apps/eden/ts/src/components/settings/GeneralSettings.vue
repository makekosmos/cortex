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
              <div class="settings-row-title">Текущая папка</div>
              <div class="settings-row-desc">{{ vaultPath }}</div>
            </div>
          </div>
          <div class="settings-row-actions">
            <button class="settings-btn-secondary" type="button" @click="emit('selectVault')">
              Сменить папку
            </button>
            <button class="settings-btn-secondary" type="button" @click="handleExport">
              Экспорт в Markdown
            </button>
          </div>
        </div>
      </section>

      <section class="settings-section">
        <div class="settings-section-header">
          <h2>Проверка кода</h2>
        </div>
        <div class="settings-section-body">
          <div class="settings-row">
            <div class="settings-row-left">
              <div class="settings-row-title">Режим проверки</div>
            </div>
            <div class="settings-row-right">
              <select
                class="settings-select-inline"
                :value="s.preset"
                @change="
                  emit('settingsChange', {
                    preset: ($event.target as HTMLSelectElement)
                      .value as CodeToolsSettings['preset'],
                  })
                "
              >
                <option v-for="o in presetOptions" :key="o.value" :value="o.value">
                  {{ o.label }} — {{ o.description }}
                </option>
              </select>
            </div>
          </div>
          <div class="settings-row">
            <div class="settings-row-left">
              <div class="settings-row-title">Когда запускать</div>
            </div>
            <div class="settings-row-right">
              <select
                class="settings-select-inline"
                :value="s.lintTrigger"
                @change="
                  emit('settingsChange', {
                    lintTrigger: ($event.target as HTMLSelectElement)
                      .value as CodeToolsSettings['lintTrigger'],
                  })
                "
              >
                <option value="on_save">При сохранении</option>
                <option value="on_idle">После паузы в наборе</option>
              </select>
            </div>
          </div>
          <div class="settings-row">
            <div class="settings-row-left">
              <div class="settings-row-title">Автоформат при сохранении</div>
            </div>
            <div class="settings-row-right">
              <input
                type="checkbox"
                :checked="s.formatOnSave"
                @change="
                  emit('settingsChange', {
                    formatOnSave: ($event.target as HTMLInputElement).checked,
                  })
                "
              />
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup vapor lang="ts">
import { computed } from "vue";

const props = defineProps<{
  settings: CodeToolsSettings | null;
  vaultPath: string;
}>();

const emit = defineEmits<{
  selectVault: [];
  settingsChange: [patch: Partial<CodeToolsSettings>];
}>();

const defaultSettings: CodeToolsSettings = {
  formatOnSave: true,
  preset: "balans",
  lintTrigger: "on_idle",
};

const presetOptions: Array<{
  value: CodeToolsSettings["preset"];
  label: string;
  description: string;
}> = [
  { value: "myagkiy", label: "Мягкий", description: "Подсказывает только важное." },
  { value: "balans", label: "Баланс", description: "Оптимальный режим." },
  { value: "strogiy", label: "Строгий", description: "Максимальная проверка." },
];

const s = computed(() => props.settings ?? defaultSettings);

async function handleExport() {
  if (!window.api) return;
  const result = await window.api.exportMarkdownVault();
  if (result?.ok) {
    window.alert(`Экспортировано: ${result.exportedCount}\nПапка: ${result.outputDir}`);
  }
}
</script>
