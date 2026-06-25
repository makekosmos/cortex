<script setup lang="ts">
import { computed, shallowRef } from "vue";
import type { RaycastSnapshotNode } from "../../shared/raycast-ipc";
import {
  menuBarIsLoading,
  menuBarSections,
  menuBarTitle,
  type RaycastMenuBarItemModel,
} from "./model";

const props = defineProps<{
  root: RaycastSnapshotNode;
  sessionId: string;
}>();

const actionStatus = shallowRef<string | null>(null);

const title = computed(() => menuBarTitle(props.root));
const sections = computed(() => menuBarSections(props.root));
const isLoading = computed(() => menuBarIsLoading(props.root));
const hasItems = computed(() => sections.value.some((section) => section.items.length > 0));

async function executeItem(item: RaycastMenuBarItemModel): Promise<void> {
  const callbackId = item.node.props.__callbackId;
  if (typeof callbackId !== "string") return;

  const result = await window.kepler.raycast.action(props.sessionId, {
    type: item.node.type,
    props: item.node.props,
  });
  actionStatus.value = result.ok ? "Готово" : "Действие не выполнено";
}
</script>

<template>
  <section class="raycast-menu-bar-extra" aria-label="Raycast Menu Bar Extra">
    <header class="raycast-menu-bar-extra__header">
      <h1 class="raycast-menu-bar-extra__title">{{ title }}</h1>
      <p class="raycast-menu-bar-extra__subtitle">Меню Raycast</p>
    </header>

    <div class="raycast-menu-bar-extra__body kosmos-scroll">
      <p v-if="isLoading" class="raycast-menu-bar-extra__state">Загрузка...</p>
      <p v-else-if="!hasItems" class="raycast-menu-bar-extra__state">Пунктов меню пока нет</p>

      <section
        v-for="section in sections"
        :key="section.id"
        class="raycast-menu-bar-extra__section"
      >
        <h2 v-if="section.title" class="raycast-menu-bar-extra__section-title">
          {{ section.title }}
        </h2>
        <div class="raycast-menu-bar-extra__items">
          <template v-for="item in section.items" :key="item.id">
            <button class="raycast-menu-bar-extra__item" type="button" @click="executeItem(item)">
              <span class="raycast-menu-bar-extra__item-text">
                <span class="raycast-menu-bar-extra__item-title">{{ item.title }}</span>
                <span v-if="item.subtitle" class="raycast-menu-bar-extra__item-subtitle">
                  {{ item.subtitle }}
                </span>
              </span>
              <span
                v-if="item.children.length > 0"
                class="raycast-menu-bar-extra__chevron"
                aria-hidden="true"
              >
                >
              </span>
            </button>

            <div v-if="item.children.length > 0" class="raycast-menu-bar-extra__nested">
              <button
                v-for="child in item.children"
                :key="child.id"
                class="raycast-menu-bar-extra__item raycast-menu-bar-extra__item--nested"
                type="button"
                @click="executeItem(child)"
              >
                <span class="raycast-menu-bar-extra__item-text">
                  <span class="raycast-menu-bar-extra__item-title">{{ child.title }}</span>
                  <span v-if="child.subtitle" class="raycast-menu-bar-extra__item-subtitle">
                    {{ child.subtitle }}
                  </span>
                </span>
              </button>
            </div>
          </template>
        </div>
      </section>
    </div>

    <footer class="raycast-menu-bar-extra__footer">
      <p v-if="actionStatus" class="raycast-menu-bar-extra__status">{{ actionStatus }}</p>
    </footer>
  </section>
</template>

<style scoped>
.raycast-menu-bar-extra {
  display: flex;
  min-height: 0;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  overflow: hidden;
}

.raycast-menu-bar-extra__header {
  flex-shrink: 0;
  border-bottom: 1px solid var(--border);
  padding: 14px 16px 12px;
}

.raycast-menu-bar-extra__title,
.raycast-menu-bar-extra__subtitle,
.raycast-menu-bar-extra__section-title,
.raycast-menu-bar-extra__state,
.raycast-menu-bar-extra__status {
  margin: 0;
}

.raycast-menu-bar-extra__title {
  font-size: 15px;
  font-weight: 700;
}

.raycast-menu-bar-extra__subtitle,
.raycast-menu-bar-extra__section-title,
.raycast-menu-bar-extra__state,
.raycast-menu-bar-extra__status,
.raycast-menu-bar-extra__item-subtitle {
  color: var(--muted-foreground);
  font-size: 12px;
}

.raycast-menu-bar-extra__subtitle {
  margin-top: 3px;
}

.raycast-menu-bar-extra__body {
  min-height: 0;
  flex: 1;
  overflow: auto;
  padding: 10px;
}

.raycast-menu-bar-extra__state {
  padding: 8px 6px;
}

.raycast-menu-bar-extra__section + .raycast-menu-bar-extra__section {
  margin-top: 12px;
}

.raycast-menu-bar-extra__section-title {
  padding: 6px 8px;
  font-weight: 700;
}

.raycast-menu-bar-extra__items,
.raycast-menu-bar-extra__nested {
  display: grid;
  gap: 4px;
}

.raycast-menu-bar-extra__nested {
  margin-left: 16px;
  border-left: 1px solid var(--border);
  padding-left: 8px;
}

.raycast-menu-bar-extra__item {
  display: flex;
  width: 100%;
  min-height: 38px;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  border: 0;
  border-radius: var(--radius-input);
  background: transparent;
  color: var(--foreground);
  padding: 7px 9px;
  text-align: left;
}

.raycast-menu-bar-extra__item:hover,
.raycast-menu-bar-extra__item:focus-visible {
  background: color-mix(in srgb, var(--accent) 18%, transparent);
}

.raycast-menu-bar-extra__item--nested {
  min-height: 34px;
}

.raycast-menu-bar-extra__item-text {
  display: grid;
  min-width: 0;
  gap: 2px;
}

.raycast-menu-bar-extra__item-title,
.raycast-menu-bar-extra__item-subtitle {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.raycast-menu-bar-extra__item-title {
  font-size: 13px;
  font-weight: 600;
}

.raycast-menu-bar-extra__chevron {
  flex: 0 0 auto;
  color: var(--muted-foreground);
  font-size: 14px;
}

.raycast-menu-bar-extra__footer {
  display: flex;
  min-height: 44px;
  align-items: center;
  border-top: 1px solid var(--border);
  padding: 8px 10px;
}
</style>
