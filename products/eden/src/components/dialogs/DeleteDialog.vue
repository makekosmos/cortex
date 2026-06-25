<template>
  <Modal
    :open="!!target"
    :title="target?.kind === 'entry' ? 'Удалить заметку?' : 'Удалить папку?'"
    width="min(100%, 340px)"
    data-testid="delete-dialog"
    @close="emit('cancel')"
  >
    <p>
      Вы уверены, что хотите удалить
      {{ target?.kind === "entry" ? "заметку" : "папку" }}
      <strong>{{ target?.title }}</strong
      >?
      <template v-if="target?.kind === 'folder'">
        Удаление доступно только для пустой папки без вложенных папок и заметок.
      </template>
    </p>
    <template #footer>
      <button
        class="dialog-secondary-btn"
        data-testid="delete-dialog-cancel"
        type="button"
        @click="emit('cancel')"
      >
        Отмена
      </button>
      <button
        class="dialog-primary-btn dialog-primary-btn-danger"
        data-testid="delete-dialog-submit"
        type="button"
        @click="target && emit('confirm', target)"
      >
        Удалить
      </button>
    </template>
  </Modal>
</template>

<script setup vapor lang="ts">
import { Modal } from "@kosmos/visuals";

export interface DeleteDialogTarget {
  kind: "entry" | "folder";
  id: string;
  title: string;
}

defineProps<{
  target: DeleteDialogTarget | null;
}>();

const emit = defineEmits<{
  cancel: [];
  confirm: [target: DeleteDialogTarget];
}>();
</script>
