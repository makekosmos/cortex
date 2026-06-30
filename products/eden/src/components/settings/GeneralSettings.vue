<template>
  <div class="settings-scroll kosmos-scroll">
    <div>
      <div class="ext-section-header">Редактор</div>
      <SettingsList>
        <SettingsToggleRow
          title="Проверка орфографии"
          description="Подчёркивает слова с возможными опечатками встроенным проверщиком браузера."
          :model-value="preferences.state.spellcheckEnabled"
          data-testid="eden-spellcheck-toggle"
          @update:model-value="preferences.setSpellcheckEnabled"
        />
        <SettingsToggleRow
          title="TipTap"
          description="Включает ProseMirror/TipTap-редактор. Vim здесь отключён."
          :model-value="preferences.state.tiptapEditorEnabled"
          data-testid="eden-tiptap-toggle"
          @update:model-value="preferences.setTiptapEditorEnabled"
        />
      </SettingsList>
    </div>
    <div>
      <div class="ext-section-header">Отображаемые типы</div>
      <SettingsList>
        <SettingsToggleRow
          v-for="noteType in eden.noteTypes"
          :key="noteType.id"
          :title="noteType.name"
          :description="noteType.id"
          :model-value="isObjectTypeVisible(noteType.id)"
          @update:model-value="(value) => setObjectTypeVisible(noteType.id, value)"
        />
      </SettingsList>
      <p class="settings-row-desc-plain">Если включены все типы, Eden загружает все объекты.</p>
    </div>
  </div>
</template>

<script setup vapor lang="ts">
import { computed, onMounted, ref } from "vue";
import { SettingsList, SettingsToggleRow } from "@kosmos/visuals";
import { usePreferences } from "@/composables/usePreferences";
import { useEdenStore } from "@/store/eden";

const preferences = usePreferences();
const eden = useEdenStore();
const visibleObjectTypeIds = ref<string[]>([]);

const allObjectTypeIds = computed(() => eden.noteTypes.map((noteType) => noteType.id));

onMounted(async () => {
  visibleObjectTypeIds.value = await window.api.getEdenVisibleObjectTypeIds();
});

function visibleObjectTypeSet(): Set<string> {
  return visibleObjectTypeIds.value.length > 0
    ? new Set(visibleObjectTypeIds.value)
    : new Set(allObjectTypeIds.value);
}

function isObjectTypeVisible(typeId: string): boolean {
  return visibleObjectTypeSet().has(typeId);
}

async function setObjectTypeVisible(typeId: string, visible: boolean): Promise<void> {
  const current = visibleObjectTypeSet();
  if (visible) {
    current.add(typeId);
  } else {
    current.delete(typeId);
  }

  const next = allObjectTypeIds.value.filter((id) => current.has(id));
  const normalizedNext = next.length === allObjectTypeIds.value.length ? [] : next;
  visibleObjectTypeIds.value = await window.api.setEdenVisibleObjectTypeIds(normalizedNext);
  await eden.refreshData();
}
</script>
