import type { Meta, StoryObj } from "@storybook/vue3-vite";
import { ref } from "vue";
import DateChip from "./DateChip.vue";

const meta: Meta<typeof DateChip> = {
  title: "Inputs/DateChip",
  component: DateChip,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
  argTypes: {
    placeholder: { control: "text" },
    compact: { control: "boolean" },
  },
};
export default meta;
type Story = StoryObj<typeof DateChip>;

export const Empty: Story = {
  render: () => ({
    components: { DateChip },
    setup: () => ({ value: ref<string | null>(null) }),
    template: `<DateChip :value="value" placeholder="Без даты" @update:value="(v) => (value = v)" />`,
  }),
};

export const WithDate: Story = {
  render: () => ({
    components: { DateChip },
    setup: () => ({ value: ref<string | null>("2026-05-18") }),
    template: `<DateChip :value="value" @update:value="(v) => (value = v)" />`,
  }),
};

export const Compact: Story = {
  render: () => ({
    components: { DateChip },
    setup: () => ({ value: ref<string | null>("2026-12-31") }),
    template: `<DateChip :value="value" compact @update:value="(v) => (value = v)" />`,
  }),
};
