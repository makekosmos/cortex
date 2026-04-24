<script setup lang="ts">
import { Plus, Trash2 } from "lucide-vue-next";
import { computed, shallowRef } from "vue";
import type {
  GameProcessBinding,
  NewGameProcessBinding,
} from "../../../src/types";
import GameProcessBindingPickerModal from "./GameProcessBindingPickerModal.vue";

interface Props {
  primaryExePath: string;
  bindings: GameProcessBinding[];
  adding?: boolean;
  removingBindingId?: number | null;
}

interface Emits {
  addBindings: [bindings: NewGameProcessBinding[]];
  removeBinding: [bindingId: number];
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

const showPicker = shallowRef(false);

function normalizeBindingKey(matchType: "exe_path" | "process_name", value: string) {
  if (matchType === "exe_path") {
    return value.trim().replaceAll("/", "\\").toLowerCase();
  }
  return value.trim().toLowerCase();
}

const existingBindingKeys = computed(() => [
  normalizeBindingKey("exe_path", props.primaryExePath),
  ...props.bindings.map((binding) =>
    normalizeBindingKey(binding.match_type, binding.match_value),
  ),
]);

function handleSubmit(bindings: NewGameProcessBinding[]) {
  emit("addBindings", bindings);
  showPicker.value = false;
}
</script>

<template>
  <div class="rounded-2xl border border-border/70 bg-card/80 p-4 shadow-sm">
    <div class="mb-4 flex items-center justify-between gap-3">
      <div>
        <div class="text-base font-semibold">Процессы игры</div>
        <div class="text-xs text-muted-foreground">
          Основной executable и дополнительные привязки из usage tracker
        </div>
      </div>
      <button
        type="button"
        class="inline-flex h-10 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
        :disabled="adding"
        @click="showPicker = true"
      >
        <Plus class="h-4 w-4" />
        Добавить процесс
      </button>
    </div>

    <div class="space-y-3">
      <div class="rounded-xl border border-primary/25 bg-primary/8 p-4">
        <div class="mb-2 flex items-center gap-2">
          <span class="rounded-full border border-primary/25 px-2 py-0.5 text-[10px] uppercase tracking-wide text-primary">
            primary
          </span>
          <span class="text-sm font-medium">Исполняемый файл запуска</span>
        </div>
        <div class="break-all font-mono text-xs text-foreground/85">
          {{ primaryExePath }}
        </div>
      </div>

      <div v-if="bindings.length === 0" class="rounded-xl border border-border/70 bg-background/30 px-4 py-3 text-sm text-muted-foreground">
        Дополнительные привязки ещё не добавлены.
      </div>

      <div v-else class="space-y-2">
        <div
          v-for="binding in bindings"
          :key="binding.id"
          class="flex items-start justify-between gap-3 rounded-xl border border-border/70 bg-background/30 px-4 py-3"
        >
          <div class="min-w-0">
            <div class="mb-2 flex items-center gap-2">
              <span class="rounded-full border border-border/60 px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground">
                {{ binding.match_type === "exe_path" ? "path" : "name" }}
              </span>
            </div>
            <div
              :class="
                binding.match_type === 'exe_path'
                  ? 'break-all font-mono text-xs text-foreground/85'
                  : 'text-sm text-foreground/90'
              "
            >
              {{ binding.match_value }}
            </div>
          </div>

          <button
            type="button"
            class="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-red-500/25 bg-red-500/10 text-red-300 transition-colors hover:bg-red-500/15 disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="adding || removingBindingId === binding.id"
            @click="emit('removeBinding', binding.id)"
          >
            <Trash2 class="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>

    <GameProcessBindingPickerModal
      v-model:open="showPicker"
      :existing-binding-keys="existingBindingKeys"
      :submitting="adding"
      @submit="handleSubmit"
    />
  </div>
</template>
