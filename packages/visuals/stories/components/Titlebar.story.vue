<script setup lang="ts">
import { Settings, Inbox } from "lucide-vue-next";
import Titlebar from "../../components/Titlebar.vue";
import TitlebarHistoryControls from "../../components/TitlebarHistoryControls.vue";
import StatusDot from "../../components/StatusDot.vue";
</script>

<template>
  <Story title="Titlebar" group="patterns" :layout="{ type: 'single', iframe: true }">
    <Variant title="Windows — title только">
      <Titlebar platform="windows" title="Kepler" />
    </Variant>

    <Variant title="macOS — с safe-area">
      <Titlebar platform="mac" title="Eden" />
    </Variant>

    <Variant title="Linux">
      <Titlebar platform="linux" title="Horologion" />
    </Variant>

    <Variant title="Со слотами (leading / trailing)">
      <Titlebar platform="windows" title="Delphi">
        <template #leading>
          <button type="button" class="icon-btn" aria-label="Inbox">
            <Inbox :size="16" />
          </button>
        </template>
        <template #trailing>
          <StatusDot tone="success" label="Подключено" />
          <button type="button" class="icon-btn" aria-label="Settings">
            <Settings :size="16" />
          </button>
        </template>
      </Titlebar>
    </Variant>

    <Variant title="С центральными history-контролами (Eden-стиль)">
      <Titlebar platform="mac">
        <template #leading>
          <span class="brand">Eden</span>
        </template>
        <template #center>
          <TitlebarHistoryControls />
        </template>
        <template #trailing>
          <StatusDot tone="neutral" label="Idle" />
        </template>
      </Titlebar>
    </Variant>
  </Story>
</template>

<style scoped>
.icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 8px;
  border: none;
  background: transparent;
  color: var(--sidebar-foreground);
  cursor: pointer;
  -webkit-app-region: no-drag;
}
.icon-btn:hover {
  background: color-mix(in srgb, var(--sidebar-foreground) 10%, transparent);
}
.brand {
  font-weight: 600;
  letter-spacing: -0.01em;
}
</style>
