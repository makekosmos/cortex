<template>
  <div v-if="isOpen" class="dialog-backdrop" data-testid="folder-dialog" @click="emit('close')">
    <div class="dialog-card" @click.stop>
      <div class="dialog-header">
        <div>
          <p class="dialog-kicker">Новая папка</p>
          <h2 class="dialog-title">Создать папку</h2>
        </div>
        <button class="dialog-close-btn" type="button" @click="emit('close')">×</button>
      </div>
      <label class="dialog-field">
        <span>Название папки</span>
        <input
          ref="inputRef"
          class="dialog-input"
          type="text"
          placeholder="Например, Проекты"
          :value="name"
          @input="emit('nameChange', ($event.target as HTMLInputElement).value)"
        />
      </label>
      <div v-if="error" class="dialog-error">{{ error }}</div>
      <div class="dialog-actions">
        <button
          class="dialog-secondary-btn"
          data-testid="folder-dialog-cancel"
          type="button"
          @click="emit('close')"
        >
          Отмена
        </button>
        <button
          class="dialog-primary-btn"
          data-testid="folder-dialog-submit"
          type="button"
          title="Создать папку"
          :disabled="isCreating"
          @click="emit('submit')"
        >
          {{ isCreating ? "Создание..." : "Создать папку" }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup vapor lang="ts">
import { useTemplateRef } from "vue";

defineProps<{
  isOpen: boolean;
  isCreating: boolean;
  name: string;
  error: string | null;
}>();

const emit = defineEmits<{
  nameChange: [value: string];
  close: [];
  submit: [];
}>();

const inputRef = useTemplateRef<HTMLInputElement>("inputRef");

defineExpose({ inputRef });
</script>
