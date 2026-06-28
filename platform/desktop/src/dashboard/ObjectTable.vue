<script setup lang="ts">
import { computed } from "vue";
import { EmptyState } from "@kosmos/visuals";
import type { DashboardObjectRow } from "./types";
import { typeVisualFor } from "./typeVisuals";

const props = defineProps<{
  rows: DashboardObjectRow[];
  loading: boolean;
}>();

function fmtUpdatedAt(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString("ru", {
      year: "numeric",
      month: "short",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

const hasRows = computed(() => props.rows.length > 0);
</script>

<template>
  <div class="wrap">
    <EmptyState v-if="loading" title="Загрузка…" compact />
    <EmptyState v-else-if="!hasRows" title="Объекты не найдены" compact />
    <div v-else class="object-table">
      <div class="object-table__header">
        <span class="object-table__header-icon" aria-hidden="true"></span>
        <span>Название</span>
        <span>Последняя модификация</span>
      </div>
      <div class="object-table__body kosmos-scroll">
        <div
          v-for="row in rows"
          :key="row.id"
          v-memo="[row.id, row.typeName, row.typeId, row.primary, row.updatedAt]"
          class="object-row"
        >
          <span
            class="object-type-icon"
            :title="row.typeName"
            :style="{
              '--object-type-icon-from': typeVisualFor(row.typeId).from,
              '--object-type-icon-to': typeVisualFor(row.typeId).to,
            }"
            aria-hidden="true"
          >
            <component :is="typeVisualFor(row.typeId).icon" :size="14" weight="duotone" />
          </span>
          <div class="object-row__primary">{{ row.primary }}</div>
          <time class="object-table__updated" :datetime="row.updatedAt">
            {{ fmtUpdatedAt(row.updatedAt) }}
          </time>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.wrap {
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.object-table {
  display: flex;
  height: 100%;
  flex-direction: column;
  overflow: hidden;
  box-sizing: border-box;
  font-size: 13px;
}

.object-table__header {
  display: grid;
  grid-template-columns: 40px minmax(180px, 1fr) 188px;
  gap: 12px;
  padding: 8px 24px;
  background: var(--main-background-color);
  border-bottom: 1px solid var(--border-color-strong);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  color: var(--muted-foreground);
}

.object-table__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.object-row {
  display: grid;
  grid-template-columns: 40px minmax(180px, 1fr) 188px;
  gap: 12px;
  align-items: center;
  min-height: 36px;
  padding: 4px 24px;
  border-bottom: 1px solid color-mix(in srgb, var(--border-color-strong) 72%, transparent);
  color: var(--foreground);
}

.object-row:hover {
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
}

.object-type-icon {
  display: inline-grid;
  width: 22px;
  height: 22px;
  place-items: center;
  justify-self: center;
  border-radius: 6px;
  background-image: linear-gradient(
    to bottom left,
    var(--object-type-icon-from),
    var(--object-type-icon-to)
  );
  color: var(--accent-foreground);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, oklch(1 0 0) 6%, transparent);
}

.object-row__primary {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
}

.object-table__updated {
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  font-size: 12px;
  white-space: nowrap;
}

@media (max-width: 820px) {
  .object-table__header,
  .object-row {
    grid-template-columns: 36px minmax(0, 1fr) 136px;
  }

  .object-table__header {
    padding-inline: 16px;
  }

  .object-row {
    padding-inline: 16px;
  }

  .object-table__updated {
    justify-self: start;
  }
}

@media (max-width: 620px) {
  .object-table__header,
  .object-row {
    grid-template-columns: 36px minmax(0, 1fr);
  }

  .object-table__header span:nth-child(3),
  .object-table__updated {
    display: none;
  }
}
</style>
