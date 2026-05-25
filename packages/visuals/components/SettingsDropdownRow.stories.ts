import type { Meta, StoryObj } from "@storybook/vue3-vite";
import SettingsDropdownRow from "./SettingsDropdownRow.vue";

const meta: Meta<typeof SettingsDropdownRow> = {
  title: "Inputs/SettingsDropdownRow",
  component: SettingsDropdownRow,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
};
export default meta;
type Story = StoryObj<typeof SettingsDropdownRow>;

const LANGUAGE_OPTIONS = [
  { value: "auto", label: "Авто" },
  { value: "ru", label: "Русский" },
  { value: "en", label: "English" },
] as const;

export const Default: Story = {
  args: {
    title: "Язык",
    description: "Подсказка для распознавания речи.",
    modelValue: "auto",
    options: LANGUAGE_OPTIONS.slice(),
  },
  render: (args) => ({
    components: { SettingsDropdownRow },
    setup: () => ({ args }),
    data: () => ({ value: args.modelValue }),
    template: `
      <div style="width: 520px; border: 1px solid var(--border); border-radius: 8px;">
        <SettingsDropdownRow v-bind="args" v-model="value" />
      </div>
    `,
  }),
};
