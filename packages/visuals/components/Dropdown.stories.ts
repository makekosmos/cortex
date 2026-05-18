import type { Meta, StoryObj } from "@storybook/vue3-vite";
import { ref } from "vue";
import Dropdown from "./Dropdown.vue";

const meta: Meta<typeof Dropdown> = {
  title: "Inputs/Dropdown",
  component: Dropdown,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
  argTypes: {
    placeholder: { control: "text" },
    matchTriggerWidth: { control: "boolean" },
    disabled: { control: "boolean" },
  },
};
export default meta;
type Story = StoryObj<typeof Dropdown>;

const themeOptions = [
  { value: "light", label: "Светлая" },
  { value: "dark", label: "Тёмная" },
  { value: "system", label: "Системная", description: "Следовать настройкам ОС" },
];

export const Default: Story = {
  render: () => ({
    components: { Dropdown },
    setup: () => ({ value: ref<string | null>(null), options: themeOptions }),
    template: `<div style="width: 240px;"><Dropdown v-model="value" :options="options" placeholder="Выбрать тему…" /></div>`,
  }),
};

export const Preselected: Story = {
  render: () => ({
    components: { Dropdown },
    setup: () => ({ value: ref<string | null>("dark"), options: themeOptions }),
    template: `<div style="width: 240px;"><Dropdown v-model="value" :options="options" /></div>`,
  }),
};

export const WithDisabledOption: Story = {
  render: () => ({
    components: { Dropdown },
    setup: () => ({
      value: ref<string | null>(null),
      options: [
        { value: "free", label: "Бесплатный" },
        { value: "pro", label: "Pro", description: "20$/мес" },
        { value: "team", label: "Team", disabled: true, description: "Скоро" },
      ],
    }),
    template: `<div style="width: 240px;"><Dropdown v-model="value" :options="options" /></div>`,
  }),
};

export const Disabled: Story = {
  render: () => ({
    components: { Dropdown },
    setup: () => ({ value: ref<string | null>("light"), options: themeOptions }),
    template: `<div style="width: 240px;"><Dropdown v-model="value" :options="options" disabled /></div>`,
  }),
};
