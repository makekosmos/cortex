<script setup lang="ts">
import { Check, Loader2, Plus, Search, X } from "lucide-vue-next";
import { computed } from "vue";
import type { NewGameProcessBinding } from "../../../src/types";
import { useUsageProcessPicker } from "../../composables/useUsageProcessPicker";

interface Props {
  existingBindingKeys: string[];
  submitting?: boolean;
}

interface Emits {
  submit: [bindings: NewGameProcessBinding[]];
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();
const open = defineModel<boolean>("open", { required: true });

const {
  query,
  items,
  loading,
  error,
  selectedBindings,
  selectedCount,
  isSelected,
  toggleSelection,
} = useUsageProcessPicker({
  isOpen: () => open.value,
});

const existingBindingKeySet = computed(() => new Set(props.existingBindingKeys));

function formatLastSeen(value: string | null) {
  if (!value) {
    return "Неизвестно";
  }

  try {
    return new Date(value).toLocaleString("ru-RU");
  } catch {
    return value;
  }
}

function isBound(bindingKey: string) {
  return existingBindingKeySet.value.has(bindingKey);
}

function handleSubmit() {
  if (selectedBindings.value.length === 0 || props.submitting) {
    return;
  }
  emit("submit", selectedBindings.value);
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 z-[125] flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm"
      @mousedown.self="open = false"
    >
      <div class="flex max-h-[85vh] w-full max-w-3xl flex-col overflow-hidden rounded-2xl border border-border/60 bg-card/92 shadow-[0_30px_80px_rgba(8,12,24,0.55)]">
        <div class="flex items-center justify-between border-b border-border/60 px-5 py-4">
          <div>
            <h2 class="text-lg font-semibold">Привязать процессы</h2>
            <p class="text-sm text-muted-foreground">
              Последние 10 процессов и поиск по usage tracker
            </p>
          </div>
          <button
            type="button"
            class="inline-flex h-10 w-10 items-center justify-center rounded-xl border border-border/70 bg-background/50 transition-colors hover:bg-accent/60"
            @click="open = false"
          >
            <X class="h-4 w-4" />
          </button>
        </div>

        <div class="border-b border-border/60 px-5 py-4">
          <div class="relative">
            <Search class="absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <input
              v-model="query"
              data-testid="usage-process-search-input"
              placeholder="Искать по имени процесса или пути..."
              class="h-11 w-full rounded-xl border border-border/70 bg-background/40 pr-4 pl-9 text-sm outline-none"
            />
          </div>
        </div>

        <div class="min-h-0 flex-1 overflow-auto px-5 py-4">
          <div v-if="loading" class="flex items-center justify-center py-10 text-muted-foreground">
            <Loader2 class="mr-2 h-4 w-4 animate-spin" />
            Загрузка процессов...
          </div>

          <div
            v-else-if="error"
            class="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300"
          >
            {{ error }}
          </div>

          <div v-else-if="items.length === 0" class="py-10 text-center text-sm text-muted-foreground">
            {{ query.trim() ? "Ничего не найдено" : "Пока нет недавних процессов" }}
          </div>

          <div v-else class="space-y-2">
            <button
              v-for="item in items"
              :key="item.tracked_app_id"
              type="button"
              :data-testid="`usage-process-option-${item.tracked_app_id}`"
              class="flex w-full items-start gap-3 rounded-2xl border px-4 py-3 text-left transition-colors"
              :class="
                isBound(item.binding_normalized_value)
                  ? 'cursor-not-allowed border-border/50 bg-background/25 opacity-60'
                  : isSelected(item.tracked_app_id)
                    ? 'border-primary/40 bg-primary/10'
                    : 'border-border/70 bg-background/30 hover:bg-accent/50'
              "
              :disabled="isBound(item.binding_normalized_value)"
              @click="toggleSelection(item)"
            >
              <div
                class="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded border"
                :class="
                  isSelected(item.tracked_app_id)
                    ? 'border-primary bg-primary text-primary-foreground'
                    : 'border-border/70'
                "
              >
                <Check v-if="isSelected(item.tracked_app_id)" class="h-3.5 w-3.5" />
              </div>

              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <div class="truncate text-sm font-medium">{{ item.display_name }}</div>
                  <span
                    class="rounded-full border border-border/60 px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground"
                  >
                    {{ item.binding_match_type === "exe_path" ? "path" : "name" }}
                  </span>
                  <span
                    v-if="isBound(item.binding_normalized_value)"
                    class="rounded-full border border-emerald-500/35 bg-emerald-500/10 px-2 py-0.5 text-[10px] uppercase tracking-wide text-emerald-300"
                  >
                    уже привязан
                  </span>
                </div>
                <div v-if="item.process_name" class="mt-1 text-xs text-muted-foreground">
                  Процесс: {{ item.process_name }}
                </div>
                <div v-if="item.exe_path" class="mt-1 break-all font-mono text-[11px] text-foreground/75">
                  {{ item.exe_path }}
                </div>
                <div class="mt-2 text-[11px] text-muted-foreground">
                  Последняя активность: {{ formatLastSeen(item.last_seen_at) }}
                  <span class="mx-1">•</span>
                  Сессий: {{ item.session_count }}
                </div>
              </div>
            </button>
          </div>
        </div>

        <div class="flex items-center justify-between gap-3 border-t border-border/60 px-5 py-4">
          <div class="text-sm text-muted-foreground">
            Выбрано: <span class="font-medium text-foreground">{{ selectedCount }}</span>
          </div>
          <div class="flex items-center gap-2">
            <button
              type="button"
              class="inline-flex h-10 items-center rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
              @click="open = false"
            >
              Отмена
            </button>
            <button
              type="button"
              data-testid="usage-process-submit"
              class="inline-flex h-10 items-center gap-2 rounded-xl bg-primary px-4 text-sm font-medium text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="selectedCount === 0 || submitting"
              @click="handleSubmit()"
            >
              <Loader2 v-if="submitting" class="h-4 w-4 animate-spin" />
              <Plus v-else class="h-4 w-4" />
              Добавить выбранное
            </button>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>
