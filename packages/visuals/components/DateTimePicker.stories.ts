import type { Meta, StoryObj } from "@storybook/vue3-vite";
import { ref } from "vue";
import DateTimePicker from "./DateTimePicker.vue";

const meta: Meta<typeof DateTimePicker> = {
  title: "Inputs/DateTimePicker",
  component: DateTimePicker,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
  argTypes: {
    placeholder: { control: "text" },
    label: { control: "text" },
  },
};
export default meta;
type Story = StoryObj<typeof DateTimePicker>;

export const Empty: Story = {
  render: () => ({
    components: { DateTimePicker },
    setup: () => ({ value: ref<string | null>(null) }),
    template: `<div style="width: 260px;"><DateTimePicker :value="value" placeholder="Выбрать дату…" @update:value="(v) => (value = v)" /></div>`,
  }),
};

export const WithValue: Story = {
  render: () => ({
    components: { DateTimePicker },
    setup: () => ({ value: ref<string | null>(new Date(2026, 4, 18, 14, 30).toISOString()) }),
    template: `<div style="width: 260px;"><DateTimePicker :value="value" @update:value="(v) => (value = v)" /></div>`,
  }),
};

export const WithReferenceSameDay: Story = {
  render: () => ({
    components: { DateTimePicker },
    setup() {
      const now = new Date();
      const today1530 = new Date(
        now.getFullYear(),
        now.getMonth(),
        now.getDate(),
        15,
        30,
      ).toISOString();
      return {
        value: ref<string | null>(today1530),
        reference: now.toISOString(),
      };
    },
    template: `<div style="width: 220px;"><DateTimePicker :value="value" :reference="reference" @update:value="(v) => (value = v)" /></div>`,
  }),
};
