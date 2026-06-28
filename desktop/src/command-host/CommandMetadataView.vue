<script setup lang="ts">
import { computed } from "vue";
import type { CommandSnapshotNode } from "../../shared/command-ipc";
import { detailMetadataItems } from "./model-detail";

const props = defineProps<{
  detail: CommandSnapshotNode | null;
}>();

const items = computed(() => detailMetadataItems(props.detail));
</script>

<template>
  <dl v-if="items.length > 0" class="command-metadata-view" aria-label="Метаданные">
    <template v-for="item in items" :key="item.id">
      <div v-if="item.type === 'separator'" class="command-metadata-view__separator" />

      <div v-else-if="item.type === 'tags'" class="command-metadata-view__row">
        <dt v-if="item.title" class="command-metadata-view__title">{{ item.title }}</dt>
        <dd class="command-metadata-view__value command-metadata-view__value--tags">
          <span v-for="tag in item.tags" :key="tag" class="command-metadata-view__tag">
            {{ tag }}
          </span>
        </dd>
      </div>

      <div v-else class="command-metadata-view__row">
        <dt v-if="item.title" class="command-metadata-view__title">{{ item.title }}</dt>
        <dd class="command-metadata-view__value">
          <a
            v-if="item.type === 'link' && item.href"
            class="command-metadata-view__link"
            :href="item.href"
            target="_blank"
            rel="noreferrer"
          >
            {{ item.text }}
          </a>
          <span v-else>{{ item.text }}</span>
        </dd>
      </div>
    </template>
  </dl>
</template>

<style scoped>
.command-metadata-view {
  display: grid;
  gap: 10px;
  margin: 16px 0 0;
  border-top: 1px solid var(--border);
  padding-top: 14px;
}

.command-metadata-view__row {
  display: grid;
  min-width: 0;
  gap: 4px;
}

.command-metadata-view__title {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 700;
}

.command-metadata-view__value {
  min-width: 0;
  margin: 0;
  color: var(--foreground);
  font-size: 12px;
  line-height: 1.35;
  word-break: break-word;
}

.command-metadata-view__value--tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.command-metadata-view__tag {
  max-width: 100%;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  padding: 2px 7px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command-metadata-view__link {
  color: var(--accent);
  text-decoration: none;
}

.command-metadata-view__link:hover {
  text-decoration: underline;
}

.command-metadata-view__separator {
  height: 1px;
  background: var(--border);
}
</style>
