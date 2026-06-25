import type { Meta, StoryObj } from "@storybook/vue3-vite";
import { ref } from "vue";
import TimeColumn from "./TimeColumn.vue";

const meta: Meta<typeof TimeColumn> = {
  title: "Inputs/TimeColumn",
  component: TimeColumn,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
  argTypes: {
    min: { control: "number" },
    max: { control: "number" },
    step: { control: "number" },
    label: { control: "text" },
  },
};
export default meta;
type Story = StoryObj<typeof TimeColumn>;

export const Hours: Story = {
  render: () => ({
    components: { TimeColumn },
    setup: () => ({ value: ref(14) }),
    template: `<TimeColumn :value="value" :min="0" :max="23" label="Часы" @update:value="(v) => (value = v)" />`,
  }),
};

export const Minutes: Story = {
  render: () => ({
    components: { TimeColumn },
    setup: () => ({ value: ref(30) }),
    template: `<TimeColumn :value="value" :min="0" :max="59" :step="1" label="Минуты" @update:value="(v) => (value = v)" />`,
  }),
};

export const MinutesFiveMinuteStep: Story = {
  render: () => ({
    components: { TimeColumn },
    setup: () => ({ value: ref(15) }),
    template: `<TimeColumn :value="value" :min="0" :max="55" :step="5" label="Минуты (шаг 5)" @update:value="(v) => (value = v)" />`,
  }),
};

export const SideBySide: Story = {
  render: () => ({
    components: { TimeColumn },
    setup: () => ({ h: ref(9), m: ref(0) }),
    template: `
      <div style="display: flex; gap: 0.5rem;">
        <TimeColumn :value="h" :min="0" :max="23" label="Часы" @update:value="(v) => (h = v)" />
        <TimeColumn :value="m" :min="0" :max="55" :step="5" label="Минуты" @update:value="(v) => (m = v)" />
      </div>
    `,
  }),
};
