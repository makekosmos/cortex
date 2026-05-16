<script setup lang="ts">
import StatusDot from "../../components/StatusDot.vue";
</script>

<template>
  <Story title="StatusDot" group="primitives">
    <Variant title="Все тоны">
      <div class="story-canvas">
        <p class="story-label">Кликни — откроется popover с дефолтным `label`.</p>
        <div class="story-row">
          <StatusDot tone="success" label="Backend подключён, sync активен" />
          <StatusDot tone="warning" label="Сетевой fallback — sync через relay" />
          <StatusDot tone="danger" label="Не могу соединиться с kepler-backend" />
          <StatusDot tone="neutral" label="Состояние неизвестно" />
        </div>
      </div>
    </Variant>

    <Variant title="С кастомным popover-контентом">
      <div class="story-canvas">
        <p class="story-label">Slot переопределяет содержимое popover’а.</p>
        <StatusDot tone="success" label="kepler-backend">
          <div style="display:flex;flex-direction:column;gap:0.35rem;">
            <strong style="font-size:0.85rem;">kepler-backend</strong>
            <p style="margin:0;font-size:0.8rem;color:var(--muted-foreground);">
              127.0.0.1:54321 · ws · uptime 12m
            </p>
            <code style="font-size:0.72rem;color:var(--muted-foreground);">
              ark-core-rpc: 1.0.3
            </code>
          </div>
        </StatusDot>
      </div>
    </Variant>
  </Story>
</template>
