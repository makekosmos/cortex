import type { Meta, StoryObj } from "@storybook/vue3-vite";
import { ref, computed } from "vue";
import CommandPalette from "./CommandPalette.vue";

const meta: Meta<typeof CommandPalette> = {
  title: "Overlays/CommandPalette",
  component: CommandPalette,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
  argTypes: {
    placeholder: { control: "text" },
  },
};
export default meta;
type Story = StoryObj<typeof CommandPalette>;

const commands = [
  "Открыть Eden",
  "Новая заметка",
  "Открыть Delphi",
  "Новая задача",
  "Запустить фокус",
  "Открыть Dashboard",
  "Настройки",
];

export const Default: Story = {
  render: () => ({
    components: { CommandPalette },
    setup() {
      const open = ref(true);
      const query = ref("");
      const filtered = computed(() =>
        commands.filter((c) => c.toLowerCase().includes(query.value.toLowerCase())),
      );
      return { open, query, filtered };
    },
    template: `
      <div style="position: relative; min-height: 400px;">
        <button @click="open = true" style="padding: 0.5rem 1rem; border: 1px solid var(--border); border-radius: 8px;">Открыть палитру</button>
        <CommandPalette
          :open="open"
          :query="query"
          placeholder="Поиск команд…"
          @update:open="open = $event"
          @update:query="query = $event"
        >
          <template #default="{ close }">
            <button
              v-for="cmd in filtered"
              :key="cmd"
              data-cmd-item
              @click="() => { console.log('run:', cmd); close(); }"
              style="display: block; width: 100%; padding: 0.5rem 1rem; text-align: left; background: transparent; border: none; color: var(--foreground); font-size: 14px; cursor: pointer;"
            >
              {{ cmd }}
            </button>
            <div v-if="filtered.length === 0" style="padding: 1rem; text-align: center; color: var(--muted-foreground); font-size: 13px;">
              Ничего не найдено
            </div>
          </template>
        </CommandPalette>
      </div>
    `,
  }),
};

export const Closed: Story = {
  render: () => ({
    components: { CommandPalette },
    setup() {
      const open = ref(false);
      return { open };
    },
    template: `
      <div style="padding: 1rem;">
        <button @click="open = true" style="padding: 0.5rem 1rem; border: 1px solid var(--border); border-radius: 8px;">Открыть палитру (Cmd+K)</button>
        <CommandPalette :open="open" @update:open="open = $event">
          <template #default="{ close }">
            <button data-cmd-item @click="close" style="display: block; width: 100%; padding: 0.5rem 1rem; text-align: left; background: transparent; border: none; color: var(--foreground); cursor: pointer;">Закрыть</button>
          </template>
        </CommandPalette>
      </div>
    `,
  }),
};
