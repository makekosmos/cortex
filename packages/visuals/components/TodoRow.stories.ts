import type { Meta, StoryObj } from "@storybook/vue3-vite";
import TodoRow, { type TodoRowItem } from "./TodoRow.vue";

const meta: Meta<typeof TodoRow> = {
  title: "Lists/TodoRow",
  component: TodoRow,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
  argTypes: {
    draggable: { control: "boolean" },
    editable: { control: "boolean" },
  },
  decorators: [
    (Story) => ({
      components: { Story },
      template: `<div style="width: 520px; padding: 0.5rem;"><Story /></div>`,
    }),
  ],
};
export default meta;
type Story = StoryObj<typeof TodoRow>;

const base: TodoRowItem = {
  id: "1",
  title: "Закончить spec для proof loop",
  notes: "Описать AC и evidence-структуру",
};

export const Default: Story = {
  args: { todo: base },
};

export const Completed: Story = {
  args: {
    todo: { ...base, id: "2", title: "Сделано", isCompleted: true },
  },
};

export const WithSchedule: Story = {
  args: {
    todo: {
      ...base,
      id: "3",
      title: "Созвон с командой",
      notes: null,
      scheduledDate: "2026-05-20",
    },
  },
};

export const Billable: Story = {
  args: {
    todo: {
      id: "4",
      title: "Дизайн ревью",
      notes: "1h slot",
      billable: true,
      price: 120,
      scheduledDate: "2026-05-19",
    },
  },
};

export const NonEditable: Story = {
  args: {
    todo: { ...base, id: "5", title: "Trashed task", isTrashed: true },
    editable: false,
    draggable: false,
  },
};
