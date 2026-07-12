<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { FileCode2, Folder, FolderOpen, RefreshCw, X } from "@lucide/vue";
import type { AgentsDiff, AgentsEditor } from "@kosmos/ark/agents";
import { buildChangeTree } from "@/changeTree";

const props = defineProps<{
  diff: AgentsDiff | null;
  editors: AgentsEditor[];
  worktreeExists: boolean;
}>();
const emit = defineEmits<{
  close: [];
  refresh: [];
  open: [editorId: AgentsEditor["id"]];
}>();
const selectedEditor = shallowRef<AgentsEditor["id"]>("code");
const rows = computed(() => buildChangeTree(props.diff?.files ?? []));
watch(
  () => props.editors,
  (editors) => {
    if (!editors.some((editor) => editor.id === selectedEditor.value))
      selectedEditor.value = editors[0]?.id ?? "explorer";
  },
  { immediate: true },
);
</script>
<template>
  <aside class="changes">
    <header>
      <div>
        <strong>Изменения</strong><span>{{ diff?.files.length ?? 0 }} файлов</span>
      </div>
      <div>
        <button class="ghost icon-button" type="button" title="Обновить" @click="emit('refresh')">
          <RefreshCw :size="15" /></button
        ><button class="ghost icon-button" type="button" title="Закрыть" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>
    </header>
    <div class="changes__files">
      <div
        v-for="row in rows"
        :key="row.key"
        class="file-row"
        :class="`file-row--${row.kind}`"
        :style="{ paddingLeft: `${6 + row.depth * 16}px` }"
      >
        <Folder v-if="row.kind === 'directory'" :size="14" />
        <FileCode2 v-else :size="14" />
        <span>{{ row.name }}</span>
        <code v-if="row.kind === 'file'">{{ row.file.status }}</code>
      </div>
    </div>
    <pre class="diff-view">{{ diff?.unifiedDiff || "Изменений пока нет" }}</pre>
    <footer>
      <select v-model="selectedEditor" :disabled="!worktreeExists" aria-label="Редактор">
        <option v-for="editor in editors" :key="editor.id" :value="editor.id">
          {{ editor.label }}
        </option>
      </select>
      <button
        class="secondary"
        type="button"
        :disabled="!worktreeExists"
        @click="emit('open', selectedEditor)"
      >
        <FolderOpen :size="15" />Открыть в редакторе
      </button>
    </footer>
  </aside>
</template>
