<template>
  <Modal
    :open="isOpen"
    title="Создать папку"
    width="min(100%, 340px)"
    data-testid="folder-dialog"
    @close="emit('close')"
  >
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
    <template #footer>
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
    </template>
  </Modal>
</template>

<script setup vapor lang="ts">
import { useTemplateRef } from "vue";
import { Modal } from "@kepler/visuals";

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
