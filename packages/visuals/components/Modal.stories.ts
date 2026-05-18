import type { Meta, StoryObj } from "@storybook/vue3-vite";
import { ref } from "vue";
import Modal from "./Modal.vue";

const meta: Meta<typeof Modal> = {
  title: "Overlays/Modal",
  component: Modal,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
  argTypes: {
    title: { control: "text" },
    width: { control: "text" },
    closeOnBackdrop: { control: "boolean" },
  },
};
export default meta;
type Story = StoryObj<typeof Modal>;

export const Default: Story = {
  render: () => ({
    components: { Modal },
    setup: () => ({ open: ref(true) }),
    template: `
      <div>
        <button @click="open = true" style="padding: 0.5rem 1rem; border: 1px solid var(--border); border-radius: 8px;">Открыть модалку</button>
        <Modal :open="open" title="Подтвердите действие" @close="open = false">
          <p style="margin: 0 0 1rem 0;">Удалить заметку без возможности восстановления?</p>
          <div style="display: flex; gap: 0.5rem; justify-content: flex-end;">
            <button @click="open = false" style="padding: 0.4rem 0.875rem; border: 1px solid var(--border); border-radius: 6px;">Отмена</button>
            <button @click="open = false" style="padding: 0.4rem 0.875rem; border: none; border-radius: 6px; background: var(--destructive); color: white;">Удалить</button>
          </div>
        </Modal>
      </div>
    `,
  }),
};

export const NoTitle: Story = {
  render: () => ({
    components: { Modal },
    setup: () => ({ open: ref(true) }),
    template: `
      <Modal :open="open" @close="open = false">
        <p>Модалка без заголовка.</p>
      </Modal>
    `,
  }),
};

export const Wide: Story = {
  render: () => ({
    components: { Modal },
    setup: () => ({ open: ref(true) }),
    template: `
      <Modal :open="open" title="Широкая модалка" width="min(720px, 92vw)" @close="open = false">
        <p>Кастомная ширина.</p>
      </Modal>
    `,
  }),
};
