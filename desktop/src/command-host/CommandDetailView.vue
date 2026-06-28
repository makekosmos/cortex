<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import type { CommandSnapshotNode } from "../../shared/command-ipc";
import { parseCommandMarkdown } from "./markdown";
import CommandActionPanel from "./CommandActionPanel.vue";
import { detailActions, detailMarkdown, detailMetadataItems } from "./model-detail";
import CommandMetadataView from "./CommandMetadataView.vue";

const props = defineProps<{
  detail: CommandSnapshotNode | null;
  sessionId?: string;
}>();

const pushedDetail = shallowRef<CommandSnapshotNode | null>(null);
const status = shallowRef<string | null>(null);

const activeDetail = computed(() => pushedDetail.value ?? props.detail);
const markdown = computed(() => detailMarkdown(activeDetail.value).trim());
const blocks = computed(() => parseCommandMarkdown(markdown.value));
const metadataItems = computed(() => detailMetadataItems(activeDetail.value));
const actions = computed(() => (props.sessionId ? detailActions(activeDetail.value) : null));

watch(
  () => props.detail,
  () => {
    pushedDetail.value = null;
    status.value = null;
  },
);

async function executeAction(action: CommandSnapshotNode): Promise<void> {
  if (action.type === "Action.Push") {
    pushedDetail.value =
      action.children.find((child) => child.type === "Detail") ?? action.children[0] ?? null;
    status.value = "Открыто";
    return;
  }

  if (
    !props.sessionId ||
    !(
      action.type === "Action" ||
      action.type === "Action.CopyToClipboard" ||
      action.type === "Action.Paste" ||
      action.type === "Action.Pop" ||
      action.type === "Action.PopToRoot" ||
      action.type === "Action.OpenInBrowser" ||
      action.type === "Action.Open" ||
      action.type === "Action.ShowInFinder" ||
      action.type === "Action.Trash" ||
      action.type === "Action.LaunchCommand"
    )
  ) {
    return;
  }

  const result = await window.kepler.command.action(props.sessionId, {
    type: action.type,
    props: action.props,
  });
  status.value = result.ok ? actionSuccessMessage(action.type) : "Действие не выполнено";
}

function actionSuccessMessage(type: string): string {
  if (type === "Action") return "Готово";
  if (type === "Action.CopyToClipboard") return "Скопировано";
  if (type === "Action.Paste") return "Вставлено";
  if (type === "Action.Pop") return "Назад";
  if (type === "Action.PopToRoot") return "К началу";
  if (type === "Action.OpenInBrowser" || type === "Action.Open") return "Открыто";
  if (type === "Action.ShowInFinder") return "Показано";
  if (type === "Action.Trash") return "Удалено";
  if (type === "Action.LaunchCommand") return "Запущено";
  return "Готово";
}
</script>

<template>
  <section class="command-detail-view" aria-label="Детали">
    <div class="command-detail-view__body kosmos-scroll">
      <div v-if="blocks.length" class="command-detail-view__markdown">
        <template v-for="(block, index) in blocks" :key="index">
          <component
            :is="`h${block.level}`"
            v-if="block.type === 'heading'"
            class="command-detail-view__heading"
          >
            {{ block.text }}
          </component>
          <p v-else-if="block.type === 'paragraph'" class="command-detail-view__paragraph">
            {{ block.text }}
          </p>
          <ul v-else-if="block.type === 'list'" class="command-detail-view__list">
            <li v-for="(item, itemIndex) in block.items" :key="itemIndex">{{ item }}</li>
          </ul>
          <pre v-else class="command-detail-view__code">{{ block.text }}</pre>
        </template>
      </div>
      <p v-else-if="metadataItems.length === 0" class="command-detail-view__empty">Нет деталей</p>

      <CommandMetadataView :detail="activeDetail" />
    </div>

    <footer v-if="actions" class="command-detail-view__footer">
      <CommandActionPanel :panel="actions" @execute="executeAction" />
      <p v-if="status" class="command-detail-view__status">{{ status }}</p>
    </footer>
  </section>
</template>

<style scoped>
.command-detail-view {
  display: flex;
  min-width: 280px;
  max-width: 360px;
  min-height: 0;
  flex-direction: column;
  border-left: 1px solid var(--border);
  background: color-mix(in srgb, var(--background) 88%, transparent);
  overflow: hidden;
}

.command-detail-view__body {
  min-height: 0;
  flex: 1;
  overflow: auto;
  padding: 16px;
}

.command-detail-view__markdown {
  margin: 0;
  word-break: break-word;
  font-size: 13px;
  line-height: 1.55;
  color: var(--foreground);
}

.command-detail-view__heading {
  margin: 0 0 10px;
  font-size: 15px;
  line-height: 1.3;
  font-weight: 700;
}

.command-detail-view__paragraph {
  margin: 0 0 12px;
}

.command-detail-view__list {
  margin: 0 0 12px;
  padding-left: 18px;
}

.command-detail-view__code {
  margin: 0 0 12px;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--muted);
  padding: 8px;
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.45;
}

.command-detail-view__markdown :last-child {
  margin-bottom: 0;
}

.command-detail-view__empty {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 13px;
}

.command-detail-view__footer {
  display: flex;
  min-height: 44px;
  align-items: center;
  gap: 10px;
  border-top: 1px solid var(--border);
  padding: 8px 10px;
}

.command-detail-view__status {
  margin: 0;
  overflow: hidden;
  color: var(--muted-foreground);
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
}
</style>
