import type { Meta, StoryObj } from "@storybook/vue3-vite";
import StatusDot from "./StatusDot.vue";

const meta: Meta<typeof StatusDot> = {
  title: "Display/StatusDot",
  component: StatusDot,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
  argTypes: {
    tone: {
      control: "select",
      options: ["success", "warning", "danger", "neutral"],
    },
    label: { control: "text" },
    sideOffset: { control: "number" },
  },
};
export default meta;
type Story = StoryObj<typeof StatusDot>;

export const Success: Story = {
  args: { tone: "success", label: "Синхронизировано" },
};

export const Warning: Story = {
  args: { tone: "warning", label: "Очередь sync переполнена" },
};

export const Danger: Story = {
  args: { tone: "danger", label: "Backend недоступен" },
};

export const Neutral: Story = {
  args: { tone: "neutral", label: "Idle" },
};

export const AllTones: Story = {
  render: () => ({
    components: { StatusDot },
    template: `
      <div style="display: flex; gap: 0.75rem; padding: 1rem;">
        <StatusDot tone="success" label="OK" />
        <StatusDot tone="warning" label="Warning" />
        <StatusDot tone="danger" label="Error" />
        <StatusDot tone="neutral" label="Idle" />
      </div>
    `,
  }),
};
