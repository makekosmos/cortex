<script setup lang="ts">
const props = defineProps<{
  builtInTypes: NoteType[];
  customTypes: NoteType[];
  selectedTypeId: string | null;
}>();

const emit = defineEmits<{
  selectType: [noteType: NoteType];
  createType: [];
}>();
</script>

<template>
  <aside class="object-types-sidebar">
    <div class="object-types-sidebar__head">
      <div class="object-types-sidebar__title-row">
        <h2 class="object-types-sidebar__title">Типы объектов</h2>
        <button class="object-types-sidebar__new" type="button" @click="emit('createType')">
          Новый
        </button>
      </div>
    </div>

    <div class="object-types-sidebar__body object-types-items">
      <section class="object-types-sidebar__section">
        <div class="object-types-sidebar__section-title">Системные типы</div>

        <button
          v-for="noteType in builtInTypes"
          :key="noteType.id"
          class="object-types-sidebar__item object-types-item builtin"
          :class="{ active: selectedTypeId === noteType.id }"
          type="button"
          @click="emit('selectType', noteType)"
        >
          <div class="object-types-sidebar__item-main">
            <div class="object-types-sidebar__item-icon-wrap">
              <img
                class="object-types-sidebar__item-icon object-types-item-icon"
                :src="`/anytype/icon/type/default/${noteType.icon || 'document'}.svg`"
                alt=""
                width="18"
                height="18"
                draggable="false"
              />
            </div>
            <span class="object-types-sidebar__item-name object-types-item-name">{{ noteType.name }}</span>
          </div>
          <span class="object-types-sidebar__item-badge">Система</span>
        </button>
      </section>

      <section class="object-types-sidebar__section">
        <div class="object-types-sidebar__section-title">Пользовательские</div>

        <button
          v-for="noteType in customTypes"
          :key="noteType.id"
          class="object-types-sidebar__item object-types-item"
          :class="{ active: selectedTypeId === noteType.id }"
          type="button"
          @click="emit('selectType', noteType)"
        >
          <div class="object-types-sidebar__item-main">
            <div class="object-types-sidebar__item-icon-wrap">
              <img
                class="object-types-sidebar__item-icon object-types-item-icon"
                :src="`/anytype/icon/type/default/${noteType.icon || 'document'}.svg`"
                alt=""
                width="18"
                height="18"
                draggable="false"
              />
            </div>
            <span class="object-types-sidebar__item-name object-types-item-name">{{ noteType.name }}</span>
          </div>
        </button>

        <div v-if="customTypes.length === 0" class="object-types-sidebar__empty">
          Пока нет пользовательских типов
        </div>
      </section>
    </div>
  </aside>
</template>

<style scoped>
.object-types-sidebar {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  min-width: 0;
  border-right: 1px solid var(--sidebar-border);
  background: var(--sidebar-bg);
}

.object-types-sidebar__head {
  display: grid;
  gap: 12px;
  padding: 18px 16px 12px;
  border-bottom: 1px solid var(--sidebar-border);
}

.object-types-sidebar__title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.object-types-sidebar__title {
  margin: 0;
  color: var(--sidebar-foreground);
  font-size: 22px;
  line-height: 1.06;
  font-weight: 600;
  letter-spacing: -0.04em;
}

.object-types-sidebar__new {
  min-height: 30px;
  padding: 0 12px;
  border: 1px solid var(--sidebar-border);
  border-radius: 999px;
  background: var(--sidebar-surface);
  color: var(--sidebar-surface-foreground);
  font-size: 12px;
  font-weight: 600;
}

.object-types-sidebar__body {
  overflow-y: auto;
  padding: 12px 10px 16px;
}

.object-types-sidebar__section {
  display: grid;
  gap: 4px;
}

.object-types-sidebar__section + .object-types-sidebar__section {
  margin-top: 16px;
}

.object-types-sidebar__section-title {
  padding: 6px 10px;
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.object-types-sidebar__item {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  min-height: 44px;
  padding: 0 12px;
  border: 1px solid transparent;
  border-radius: 12px;
  background: transparent;
  color: var(--sidebar-foreground);
  text-align: left;
  transition: border-color 0.16s ease;
}

.object-types-sidebar__item::before {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: transparent;
  transition: background-color 0.16s ease;
  pointer-events: none;
}

.object-types-sidebar__item:hover::before,
.object-types-sidebar__item.active::before {
  background: var(--sidebar-surface);
}

.object-types-sidebar__item.active {
  border-color: var(--sidebar-border);
}

.object-types-sidebar__item-main {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.object-types-sidebar__item-icon-wrap {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border-radius: 10px;
  background: var(--background);
  border: 1px solid var(--sidebar-border);
  flex-shrink: 0;
}

.object-types-sidebar__item-icon {
  filter: invert(1);
  opacity: 0.76;
}

.object-types-sidebar__item-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  font-weight: 500;
}

.object-types-sidebar__item-badge {
  position: relative;
  z-index: 1;
  flex-shrink: 0;
  min-height: 22px;
  padding: 0 8px;
  border: 1px solid var(--sidebar-border);
  border-radius: 999px;
  background: var(--background);
  color: var(--muted-foreground);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.04em;
  line-height: 20px;
  text-transform: uppercase;
}

.object-types-sidebar__empty {
  padding: 12px 10px;
  color: var(--muted-foreground);
  font-size: 12px;
}
</style>
