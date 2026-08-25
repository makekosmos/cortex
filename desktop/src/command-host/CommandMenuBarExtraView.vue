<script setup lang="ts">
import { computed, shallowRef } from "vue";
import type { CommandSnapshotNode } from "../../shared/command-ipc";
import { isString } from "../shared/runtimeGuards";
import { menuBarIsLoading, menuBarSections, menuBarTitle } from "./model-menu";
import type { CommandMenuBarItemModel } from "./model";

const props = defineProps<{
  root: CommandSnapshotNode;
  sessionId: string;
}>();

const actionStatus = shallowRef<string | null>(null);

const title = computed(() => menuBarTitle(props.root));
const sections = computed(() => menuBarSections(props.root));
const isLoading = computed(() => menuBarIsLoading(props.root));
const hasItems = computed(() => sections.value.some((section) => section.items.length > 0));

async function executeItem(item: CommandMenuBarItemModel): Promise<void> {
  const callbackId = item.node.props.__callbackId;
  if (!isString(callbackId)) return;

  const result = await window.kepler.command.action(props.sessionId, {
    type: item.node.type,
    props: item.node.props,
  });
  actionStatus.value = result.ok ? "Готово" : "Действие не выполнено";
}
</script>

<template>
  <section class="command-menu-bar-extra" aria-label="Command Menu Bar Extra">
    <header class="command-menu-bar-extra__header">
      <h1 class="command-menu-bar-extra__title">{{ title }}</h1>
      <p class="command-menu-bar-extra__subtitle">Меню Command</p>
    </header>

    <div class="command-menu-bar-extra__body kosmos-scroll">
      <p v-if="isLoading" class="command-menu-bar-extra__state">Загрузка...</p>
      <p v-else-if="!hasItems" class="command-menu-bar-extra__state">Пунктов меню пока нет</p>

      <section
        v-for="section in sections"
        :key="section.id"
        class="command-menu-bar-extra__section"
      >
        <h2 v-if="section.title" class="command-menu-bar-extra__section-title">
          {{ section.title }}
        </h2>
        <div class="command-menu-bar-extra__items">
          <template v-for="item in section.items" :key="item.id">
            <button class="command-menu-bar-extra__item" type="button" @click="executeItem(item)">
              <span class="command-menu-bar-extra__item-text">
                <span class="command-menu-bar-extra__item-title">{{ item.title }}</span>
                <span v-if="item.subtitle" class="command-menu-bar-extra__item-subtitle">
                  {{ item.subtitle }}
                </span>
              </span>
              <span
                v-if="item.children.length > 0"
                class="command-menu-bar-extra__chevron"
                aria-hidden="true"
              >
                >
              </span>
            </button>

            <div v-if="item.children.length > 0" class="command-menu-bar-extra__nested">
              <button
                v-for="child in item.children"
                :key="child.id"
                class="command-menu-bar-extra__item command-menu-bar-extra__item--nested"
                type="button"
                @click="executeItem(child)"
              >
                <span class="command-menu-bar-extra__item-text">
                  <span class="command-menu-bar-extra__item-title">{{ child.title }}</span>
                  <span v-if="child.subtitle" class="command-menu-bar-extra__item-subtitle">
                    {{ child.subtitle }}
                  </span>
                </span>
              </button>
            </div>
          </template>
        </div>
      </section>
    </div>

    <footer class="command-menu-bar-extra__footer">
      <p v-if="actionStatus" class="command-menu-bar-extra__status">{{ actionStatus }}</p>
    </footer>
  </section>
</template>

<style scoped>
.command-menu-bar-extra {
  display: flex;
  min-height: 0;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  overflow: hidden;
}

.command-menu-bar-extra__header {
  flex-shrink: 0;
  border-bottom: 1px solid var(--border);
  padding: 14px 16px 12px;
}

.command-menu-bar-extra__title,
.command-menu-bar-extra__subtitle,
.command-menu-bar-extra__section-title,
.command-menu-bar-extra__state,
.command-menu-bar-extra__status {
  margin: 0;
}

.command-menu-bar-extra__title {
  font-size: 15px;
  font-weight: 700;
}

.command-menu-bar-extra__subtitle,
.command-menu-bar-extra__section-title,
.command-menu-bar-extra__state,
.command-menu-bar-extra__status,
.command-menu-bar-extra__item-subtitle {
  color: var(--muted-foreground);
  font-size: 12px;
}

.command-menu-bar-extra__subtitle {
  margin-top: 3px;
}

.command-menu-bar-extra__body {
  min-height: 0;
  flex: 1;
  overflow: auto;
  padding: 10px;
}

.command-menu-bar-extra__state {
  padding: 8px 6px;
}

.command-menu-bar-extra__section + .command-menu-bar-extra__section {
  margin-top: 12px;
}

.command-menu-bar-extra__section-title {
  padding: 6px 8px;
  font-weight: 700;
}

.command-menu-bar-extra__items,
.command-menu-bar-extra__nested {
  display: grid;
  gap: 4px;
}

.command-menu-bar-extra__nested {
  margin-left: 16px;
  border-left: 1px solid var(--border);
  padding-left: 8px;
}

.command-menu-bar-extra__item {
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

.command-menu-bar-extra__item:hover,
.command-menu-bar-extra__item:focus-visible {
  background: color-mix(in srgb, var(--accent) 18%, transparent);
}

.command-menu-bar-extra__item--nested {
  min-height: 34px;
}

.command-menu-bar-extra__item-text {
  display: grid;
  min-width: 0;
  gap: 2px;
}

.command-menu-bar-extra__item-title,
.command-menu-bar-extra__item-subtitle {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command-menu-bar-extra__item-title {
  font-size: 13px;
  font-weight: 600;
}

.command-menu-bar-extra__chevron {
  flex: 0 0 auto;
  color: var(--muted-foreground);
  font-size: 14px;
}

.command-menu-bar-extra__footer {
  display: flex;
  min-height: 44px;
  align-items: center;
  border-top: 1px solid var(--border);
  padding: 8px 10px;
}
</style>
