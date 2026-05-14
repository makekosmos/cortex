<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch, nextTick } from "vue";

/**
 * Контекст-меню (ПКМ-меню) для desktop-приложений Kosmos.
 *
 * Использование:
 *
 * ```vue
 * <li @contextmenu.prevent="(e) => menu.open(e, payload)">…</li>
 *
 * <ContextMenu
 *   :open="menu.isOpen.value"
 *   :x="menu.x.value"
 *   :y="menu.y.value"
 *   @close="menu.close"
 * >
 *   <ContextMenuItem @click="remove(menu.payload.value); menu.close()">
 *     Удалить
 *   </ContextMenuItem>
 * </ContextMenu>
 * ```
 *
 * Хелпер `useContextMenu<T>()` экспортируется из `@kosmos/visuals`.
 */

interface Props {
  open: boolean;
  x: number;
  y: number;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  close: [];
}>();

const root = ref<HTMLElement | null>(null);
const finalX = ref(0);
const finalY = ref(0);

// После рендера прижимаем меню к границам окна, чтобы не вылезало.
watch(
  () => [props.open, props.x, props.y],
  async () => {
    if (!props.open) return;
    await nextTick();
    const el = root.value;
    if (!el) return;
    const { innerWidth, innerHeight } = window;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    finalX.value = Math.min(props.x, innerWidth - w - 8);
    finalY.value = Math.min(props.y, innerHeight - h - 8);
  },
  { immediate: true },
);

const style = computed(() => ({
  top: `${finalY.value}px`,
  left: `${finalX.value}px`,
}));

function onDocumentPointerDown(e: PointerEvent) {
  if (!props.open) return;
  const target = e.target;
  if (target instanceof Node && root.value?.contains(target)) return;
  emit("close");
}

function onDocumentKey(e: KeyboardEvent) {
  if (e.key === "Escape" && props.open) emit("close");
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      document.addEventListener("pointerdown", onDocumentPointerDown);
      document.addEventListener("keydown", onDocumentKey);
    } else {
      document.removeEventListener("pointerdown", onDocumentPointerDown);
      document.removeEventListener("keydown", onDocumentKey);
    }
  },
);

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocumentPointerDown);
  document.removeEventListener("keydown", onDocumentKey);
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="props.open"
      ref="root"
      class="kosmos-context-menu"
      role="menu"
      :style="style"
    >
      <slot />
    </div>
  </Teleport>
</template>

<style scoped>
.kosmos-context-menu {
  position: fixed;
  z-index: 9999;
  min-width: 180px;
  padding: 0.25rem;
  background: var(--popover, var(--background));
  color: var(--popover-foreground, var(--foreground));
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.85);
  corner-shape: var(--corner-shape);
  box-shadow:
    0 12px 32px rgb(0 0 0 / 28%),
    0 4px 12px rgb(0 0 0 / 14%);
}
</style>
