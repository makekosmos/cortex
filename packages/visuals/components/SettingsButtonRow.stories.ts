import type { Meta, StoryObj } from "@storybook/vue3-vite";
import SettingsButtonRow from "./SettingsButtonRow.vue";

const meta: Meta<typeof SettingsButtonRow> = {
  title: "Inputs/SettingsButtonRow",
  component: SettingsButtonRow,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
};
export default meta;
type Story = StoryObj<typeof SettingsButtonRow>;

export const Default: Story = {
  args: {
    title: "Открыть папку логов",
    description: "Откроет в файловом менеджере.",
    buttonLabel: "Открыть",
    variant: "ghost",
  },
  render: (args) => ({
    components: { SettingsButtonRow },
    setup: () => ({ args }),
    template: `
      <div style="width: 520px; border: 1px solid var(--border); border-radius: 8px;">
        <SettingsButtonRow v-bind="args" />
      </div>
    `,
  }),
};
