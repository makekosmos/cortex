<template>
  <div v-if="target" class="dialog-backdrop" data-testid="delete-dialog" @click="emit('cancel')">
    <div class="dialog-card" @click.stop>
      <div class="dialog-header">
        <div>
          <p class="dialog-kicker">Удаление</p>
          <h2 class="dialog-title">
            {{ target.kind === "entry" ? "Удалить заметку?" : "Удалить папку?" }}
          </h2>
        </div>
        <button class="dialog-close-btn" type="button" @click="emit('cancel')">×</button>
      </div>
      <div class="dialog-field">
        <p>
          Вы уверены, что хотите удалить {{ target.kind === "entry" ? "заметку" : "папку" }}
          <strong>{{ target.title }}</strong
          >?
          <template v-if="target.kind === 'folder'">
            Удаление доступно только для пустой папки без вложенных папок и заметок.
          </template>
        </p>
      </div>
      <div class="dialog-actions">
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
          @click="emit('confirm', target)"
        >
          Удалить
        </button>
      </div>
    </div>
  </div>
</template>

<script setup vapor lang="ts">
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
