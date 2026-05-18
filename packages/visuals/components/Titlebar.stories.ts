import type { Meta, StoryObj } from "@storybook/vue3-vite";
import Titlebar from "./Titlebar.vue";

const meta: Meta<typeof Titlebar> = {
  title: "Window/Titlebar",
  component: Titlebar,
  tags: ["autodocs"],
  argTypes: {
    platform: { control: "select", options: ["mac", "windows", "linux"] },
    title: { control: "text" },
  },
  parameters: { layout: "fullscreen" },
};
export default meta;
type Story = StoryObj<typeof Titlebar>;

export const Windows: Story = {
  args: { platform: "windows", title: "Kepler" },
};

export const Mac: Story = {
  args: { platform: "mac", title: "Eden" },
};

export const WithSlots: Story = {
  args: { platform: "windows" },
  render: (args) => ({
    components: { Titlebar },
    setup: () => ({ args }),
    template: `
      <Titlebar v-bind="args">
        <template #leading><span style="font-size: 13px; color: var(--muted-foreground); padding-left: 0.75rem;">Eden</span></template>
        <template #center><span style="font-size: 13px; font-weight: 500;">Сегодняшняя заметка</span></template>
        <template #trailing><span style="font-size: 12px; color: var(--muted-foreground); padding-right: 0.75rem;">⚙</span></template>
      </Titlebar>
    `,
  }),
};
