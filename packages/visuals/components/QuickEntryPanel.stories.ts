import type { Meta, StoryObj } from "@storybook/vue3-vite";
import { ref } from "vue";
import QuickEntryPanel from "./QuickEntryPanel.vue";

const meta: Meta<typeof QuickEntryPanel> = {
  title: "Overlays/QuickEntryPanel",
  component: QuickEntryPanel,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
};
export default meta;
type Story = StoryObj<typeof QuickEntryPanel>;

const projects = [
  { id: "p1", title: "Kosmos", billable: false },
  { id: "p2", title: "Клиент Acme", billable: true },
  { id: "p3", title: "Личные", billable: false },
];

export const Default: Story = {
  render: () => ({
    components: { QuickEntryPanel },
    setup() {
      const open = ref(true);
      function onSave(payload: unknown) {
        console.log("save", payload);
        open.value = false;
      }
      return { open, projects, onSave };
    },
    template: `
      <div>
        <button @click="open = true" style="padding: 0.5rem 1rem; border: 1px solid var(--border); border-radius: 8px;">Открыть quick entry</button>
        <QuickEntryPanel :open="open" :projects="projects" @update:open="open = $event" @save="onSave" />
      </div>
    `,
  }),
};

export const WithDefaults: Story = {
  render: () => ({
    components: { QuickEntryPanel },
    setup: () => ({
      open: ref(true),
      projects,
      onSave: (p: unknown) => console.log(p),
    }),
    template: `
      <QuickEntryPanel
        :open="open"
        :projects="projects"
        default-scheduled-date="2026-05-20"
        default-project-id="p2"
        @update:open="open = $event"
        @save="onSave"
      />
    `,
  }),
};
