import type { Meta, StoryObj } from "@storybook/vue3-vite";
import { ref } from "vue";
import Calendar from "./Calendar.vue";

const meta: Meta<typeof Calendar> = {
  title: "Inputs/Calendar",
  component: Calendar,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
  argTypes: {
    today: { control: "text" },
  },
};
export default meta;
type Story = StoryObj<typeof Calendar>;

export const Empty: Story = {
  render: () => ({
    components: { Calendar },
    setup: () => ({ value: ref<string | null>(null) }),
    template: `<div style="width: 320px;"><Calendar :value="value" @pick="(iso) => (value = iso)" /></div>`,
  }),
};

export const Preselected: Story = {
  render: () => ({
    components: { Calendar },
    setup: () => ({ value: ref<string | null>("2026-05-18") }),
    template: `<div style="width: 320px;"><Calendar :value="value" @pick="(iso) => (value = iso)" /></div>`,
  }),
};
