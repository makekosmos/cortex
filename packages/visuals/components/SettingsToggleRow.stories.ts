import type { Meta, StoryObj } from "@storybook/vue3-vite";
import SettingsToggleRow from "./SettingsToggleRow.vue";

const meta: Meta<typeof SettingsToggleRow> = {
  title: "Inputs/SettingsToggleRow",
  component: SettingsToggleRow,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
};
export default meta;
type Story = StoryObj<typeof SettingsToggleRow>;

export const Default: Story = {
  args: {
    title: "Автостарт работы",
    description: "Запускать рабочее время сразу после перерыва.",
    modelValue: false,
  },
  render: (args) => ({
    components: { SettingsToggleRow },
    setup: () => ({ args }),
    data: () => ({ value: args.modelValue }),
    template: `
      <div style="width: 520px; border: 1px solid var(--border); border-radius: 8px;">
        <SettingsToggleRow v-bind="args" v-model="value" />
      </div>
    `,
  }),
};
