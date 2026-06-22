<template>
  <kbd class="kbd-key"><slot /></kbd>
</template>

<style scoped>
.kbd-key {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 16px;
  height: 16px;
  padding: 0 5px;
  border: 1px solid color-mix(in srgb, var(--foreground) 22%, transparent);
  border-radius: 4px;
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
  font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", monospace;
  font-size: 10px;
  font-style: normal;
  line-height: normal;
  white-space: nowrap;
}
</style>
