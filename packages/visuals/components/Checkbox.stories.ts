import type { Meta, StoryObj } from "@storybook/vue3-vite";
import Checkbox from "./Checkbox.vue";

const meta: Meta<typeof Checkbox> = {
  title: "Inputs/Checkbox",
  component: Checkbox,
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
  },
  argTypes: {
    modelValue: { control: "boolean" },
    disabled: { control: "boolean" },
    ariaLabel: { control: "text" },
  },
};
export default meta;
type Story = StoryObj<typeof Checkbox>;

export const Default: Story = {
  args: { modelValue: false, ariaLabel: "Отметить" },
};

export const Checked: Story = {
  args: { modelValue: true, ariaLabel: "Снять отметку" },
};

export const Disabled: Story = {
  args: { modelValue: false, disabled: true, ariaLabel: "Заблокировано" },
};

export const DisabledChecked: Story = {
  args: { modelValue: true, disabled: true, ariaLabel: "Заблокировано" },
};

// Демонстрация override акцента через CSS-var — например, Eden использует
// `--kosmos-checkbox-accent: #ff5c00` (brand orange).
export const EdenAccent: Story = {
  args: { modelValue: true, ariaLabel: "Готово" },
  decorators: [
    () => ({
      template: `<div style="--kosmos-checkbox-accent: #ff5c00;"><story></story></div>`,
    }),
  ],
};
