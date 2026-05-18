import type { Meta, StoryObj } from "@storybook/vue3-vite";
import ContextMenuItem from "./ContextMenuItem.vue";

const meta: Meta<typeof ContextMenuItem> = {
  title: "Overlays/ContextMenuItem",
  component: ContextMenuItem,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
  argTypes: {
    destructive: { control: "boolean" },
    disabled: { control: "boolean" },
  },
  decorators: [
    (Story) => ({
      components: { Story },
      template: `
        <div style="width: 220px; padding: 0.25rem; background: var(--popover, var(--background)); border: 1px solid var(--border); border-radius: 8px;">
          <Story />
        </div>
      `,
    }),
  ],
};
export default meta;
type Story = StoryObj<typeof ContextMenuItem>;

export const Default: Story = {
  render: (args) => ({
    components: { ContextMenuItem },
    setup: () => ({ args }),
    template: `<ContextMenuItem v-bind="args">Открыть</ContextMenuItem>`,
  }),
};

export const Destructive: Story = {
  args: { destructive: true },
  render: Default.render,
};

export const Disabled: Story = {
  args: { disabled: true },
  render: (args) => ({
    components: { ContextMenuItem },
    setup: () => ({ args }),
    template: `<ContextMenuItem v-bind="args">Недоступно</ContextMenuItem>`,
  }),
};
