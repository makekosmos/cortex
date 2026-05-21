import type { Meta, StoryObj } from "@storybook/vue3-vite";
import { Inbox } from "@lucide/vue";
import EmptyState from "./EmptyState.vue";

const meta: Meta<typeof EmptyState> = {
  title: "Display/EmptyState",
  component: EmptyState,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
  argTypes: {
    title: { control: "text" },
    description: { control: "text" },
    compact: { control: "boolean" },
  },
};
export default meta;
type Story = StoryObj<typeof EmptyState>;

export const Default: Story = {
  args: { title: "Нет заметок" },
};

export const WithDescription: Story = {
  args: {
    title: "Нет заметок",
    description: "Создайте первую заметку, чтобы начать вести дневник.",
  },
};

export const WithIconAndAction: Story = {
  args: {
    title: "Корзина пуста",
    description: "Удалённые заметки появятся здесь.",
  },
  render: (args) => ({
    components: { EmptyState },
    setup: () => ({ args, Inbox }),
    template: `
      <EmptyState v-bind="args">
        <template #icon><Inbox :size="36" :stroke-width="1.5" /></template>
        <template #action>
          <button style="padding: 0.5rem 1rem; border-radius: 8px; background: var(--accent); color: var(--accent-foreground); border: none; cursor: pointer;">
            Создать заметку
          </button>
        </template>
      </EmptyState>
    `,
  }),
};

export const Compact: Story = {
  args: {
    title: "Ничего не найдено",
    description: "Попробуйте другой запрос.",
    compact: true,
  },
};
