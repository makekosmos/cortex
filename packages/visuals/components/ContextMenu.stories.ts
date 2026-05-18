import type { Meta, StoryObj } from "@storybook/vue3-vite";
import { ref } from "vue";
import ContextMenu from "./ContextMenu.vue";
import ContextMenuItem from "./ContextMenuItem.vue";

const meta: Meta<typeof ContextMenu> = {
  title: "Overlays/ContextMenu",
  component: ContextMenu,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
};
export default meta;
type Story = StoryObj<typeof ContextMenu>;

export const Default: Story = {
  render: () => ({
    components: { ContextMenu, ContextMenuItem },
    setup() {
      const open = ref(false);
      const x = ref(0);
      const y = ref(0);
      function show(e: MouseEvent) {
        e.preventDefault();
        x.value = e.clientX;
        y.value = e.clientY;
        open.value = true;
      }
      return { open, x, y, show, close: () => (open.value = false) };
    },
    template: `
      <div
        @contextmenu="show"
        style="display: flex; align-items: center; justify-content: center; width: 100%; height: 240px; border: 2px dashed var(--border); border-radius: 8px; color: var(--muted-foreground);"
      >
        ПКМ для открытия меню
        <ContextMenu :open="open" :x="x" :y="y" @close="close">
          <ContextMenuItem @click="close">Открыть</ContextMenuItem>
          <ContextMenuItem @click="close">Переименовать</ContextMenuItem>
          <ContextMenuItem destructive @click="close">Удалить</ContextMenuItem>
        </ContextMenu>
      </div>
    `,
  }),
};

export const AlwaysOpen: Story = {
  render: () => ({
    components: { ContextMenu, ContextMenuItem },
    setup: () => ({ open: ref(true), x: ref(60), y: ref(60) }),
    template: `
      <div style="position: relative; width: 320px; height: 200px;">
        <ContextMenu :open="open" :x="x" :y="y" @close="() => {}">
          <ContextMenuItem>Копировать</ContextMenuItem>
          <ContextMenuItem>Вставить</ContextMenuItem>
          <ContextMenuItem disabled>Вырезать (disabled)</ContextMenuItem>
          <ContextMenuItem destructive>Удалить</ContextMenuItem>
        </ContextMenu>
      </div>
    `,
  }),
};
