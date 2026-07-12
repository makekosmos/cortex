<script setup lang="ts">
import { shallowRef } from "vue";
import { Send, Square } from "@lucide/vue";
defineProps<{ running: boolean }>();
const emit = defineEmits<{ send: [text: string]; interrupt: [] }>();
const text = shallowRef("");
function submit(): void {
  const value = text.value.trim();
  if (!value) return;
  emit("send", value);
  text.value = "";
}
</script>
<template>
  <form class="composer" @submit.prevent="submit">
    <textarea
      v-model="text"
      rows="2"
      placeholder="Написать уточнение…"
      @keydown.ctrl.enter.prevent="submit"
    />
    <div class="composer__actions">
      <span>Ctrl + Enter</span
      ><button
        v-if="running"
        class="secondary icon-button"
        type="button"
        title="Остановить"
        @click="emit('interrupt')"
      >
        <Square :size="15" /></button
      ><button class="primary icon-button" type="submit" :disabled="!text.trim()">
        <Send :size="16" />
      </button>
    </div>
  </form>
</template>
