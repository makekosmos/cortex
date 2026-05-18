import type { Meta, StoryObj } from "@storybook/vue3";
import Toggle from "./Toggle.vue";

const meta: Meta<typeof Toggle> = {
  title: "Components/Toggle",
  component: Toggle,
  tags: ["autodocs"],
  argTypes: {
    modelValue: { control: "boolean" },
    disabled: { control: "boolean" },
    label: { control: "text" },
    ariaLabel: { control: "text" },
  },
};
export default meta;
type Story = StoryObj<typeof Toggle>;

export const Default: Story = {
  args: { modelValue: false },
};

export const Checked: Story = {
  args: { modelValue: true },
};

export const WithLabel: Story = {
  args: { modelValue: true, label: "Автосохранение" },
};

export const Disabled: Story = {
  args: { modelValue: true, disabled: true, label: "Заблокировано" },
};
