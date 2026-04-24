<script setup lang="ts">
import { Globe, Image as ImageIcon, Loader2, Save, X } from "lucide-vue-next";

const name = defineModel<string>("name", { required: true });
const description = defineModel<string>("description", { required: true });
const backgroundImage = defineModel<string>("backgroundImage", { required: true });
const coverImage = defineModel<string>("coverImage", { required: true });

defineProps<{
  open: boolean;
  gameName: string;
  saving: boolean;
}>();

const emit = defineEmits<{
  close: [];
  save: [];
  searchImage: [payload: { query: string; target: "background" | "cover" }];
}>();
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 z-[122] flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm"
      @mousedown.self="emit('close')"
    >
      <div class="flex max-h-[90vh] w-full max-w-2xl flex-col overflow-hidden rounded-xl border bg-card shadow-2xl">
        <div class="flex items-center justify-between border-b p-6">
          <h2 class="text-xl font-bold">Редактировать игру</h2>
          <div class="flex items-center gap-2">
            <button
              type="button"
              class="inline-flex items-center gap-2 rounded-xl bg-primary px-4 py-2 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white"
              :disabled="saving"
              @click="emit('save')"
            >
              <Loader2 v-if="saving" class="h-4 w-4 animate-spin" />
              <Save v-else class="h-4 w-4" />
              Сохранить
            </button>
            <button
              type="button"
              class="inline-flex h-10 w-10 items-center justify-center rounded-xl border border-border/70 bg-card/80 transition-colors hover:bg-accent/70"
              @click="emit('close')"
            >
              <X class="h-5 w-5" />
            </button>
          </div>
        </div>

        <div class="flex-1 overflow-auto p-6">
          <div class="space-y-6">
            <div class="space-y-2">
              <label class="text-sm font-medium" for="edit-name">Название</label>
              <input
                id="edit-name"
                :value="name"
                class="h-11 w-full rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
                @input="name = ($event.target as HTMLInputElement).value"
              />
            </div>

            <div class="space-y-2">
              <label class="text-sm font-medium" for="edit-description">Описание</label>
              <textarea
                id="edit-description"
                :value="description"
                class="min-h-[120px] w-full rounded-xl border border-border/70 bg-card/70 px-4 py-3 text-sm outline-none"
                @input="description = ($event.target as HTMLTextAreaElement).value"
              />
            </div>

            <div class="space-y-5">
              <div class="space-y-2">
                <label class="block text-sm font-medium" for="edit-background-url">
                  URL фона
                </label>
                <div class="flex gap-2">
                  <input
                    id="edit-background-url"
                    :value="backgroundImage"
                    placeholder="https://..."
                    class="h-11 flex-1 rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
                    @input="backgroundImage = ($event.target as HTMLInputElement).value"
                  />
                  <button
                    type="button"
                    class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-4 py-2 text-sm transition-colors hover:bg-accent/70"
                    @click="emit('searchImage', { query: name || gameName, target: 'background' })"
                  >
                    <Globe class="h-4 w-4" />
                    Google
                  </button>
                </div>
                <div class="aspect-video overflow-hidden rounded-xl border border-border/70 bg-background/40">
                  <img
                    v-if="backgroundImage"
                    :src="backgroundImage"
                    :alt="`${name || gameName} background`"
                    class="h-full w-full object-cover"
                  />
                  <div v-else class="flex h-full items-center justify-center text-muted-foreground">
                    <ImageIcon class="mr-2 h-8 w-8 opacity-50" />
                    Нет изображения
                  </div>
                </div>
              </div>

              <div class="space-y-2">
                <label class="block text-sm font-medium" for="edit-cover-url">
                  URL обложки
                </label>
                <div class="flex gap-2">
                  <input
                    id="edit-cover-url"
                    :value="coverImage"
                    placeholder="https://..."
                    class="h-11 flex-1 rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
                    @input="coverImage = ($event.target as HTMLInputElement).value"
                  />
                  <button
                    type="button"
                    class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-4 py-2 text-sm transition-colors hover:bg-accent/70"
                    @click="emit('searchImage', { query: name || gameName, target: 'cover' })"
                  >
                    <Globe class="h-4 w-4" />
                    Google
                  </button>
                </div>
                <div class="aspect-[2/3] w-40 overflow-hidden rounded-xl border border-border/70 bg-background/40">
                  <img
                    v-if="coverImage"
                    :src="coverImage"
                    :alt="`${name || gameName} cover`"
                    class="h-full w-full object-cover"
                  />
                  <div v-else class="flex h-full items-center justify-center px-3 text-center text-xs text-muted-foreground">
                    Нет обложки
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>
