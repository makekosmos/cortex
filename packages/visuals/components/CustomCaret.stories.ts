import type { Meta, StoryObj } from "@storybook/vue3";
import CustomCaret from "./CustomCaret.vue";

const meta: Meta<typeof CustomCaret> = {
  title: "Components/CustomCaret",
  component: CustomCaret,
  tags: ["autodocs"],
  parameters: {
    docs: {
      description: {
        component:
          "Глобальный кастомный caret. Монтируется один раз на приложение, перехватывает фокус на input/textarea/contenteditable и рисует custom blinking caret через Teleport в body. Кликни в поле ниже чтобы увидеть.",
      },
    },
  },
};
export default meta;
type Story = StoryObj<typeof CustomCaret>;

export const InInputField: Story = {
  render: () => ({
    components: { CustomCaret },
    template: `
      <div style="display: flex; flex-direction: column; gap: 0.75rem; padding: 1rem; width: 420px;">
        <CustomCaret />
        <input type="text" placeholder="Кликни сюда и печатай…" style="height: 36px; padding: 0 0.75rem; border: 2px solid var(--border); border-radius: 8px; background: var(--background); color: var(--foreground); font-size: 14px;" />
        <textarea rows="4" placeholder="И сюда тоже…" style="padding: 0.5rem 0.75rem; border: 2px solid var(--border); border-radius: 8px; background: var(--background); color: var(--foreground); font-size: 14px; resize: vertical;"></textarea>
      </div>
    `,
  }),
};

export const InContentEditable: Story = {
  render: () => ({
    components: { CustomCaret },
    template: `
      <div style="padding: 1rem; width: 420px;">
        <CustomCaret />
        <div contenteditable="true" style="min-height: 120px; padding: 0.75rem; border: 2px solid var(--border); border-radius: 8px; background: var(--background); color: var(--foreground); font-size: 14px; line-height: 1.5;">
          Это contenteditable div. Кликни в любое место текста и увидишь кастомный caret вместо нативного.
        </div>
      </div>
    `,
  }),
};
