import type { Meta, StoryObj } from "@storybook/vue3-vite";
import SettingsRow from "./SettingsRow.vue";
import Toggle from "./Toggle.vue";

const meta: Meta<typeof SettingsRow> = {
  title: "Inputs/SettingsRow",
  component: SettingsRow,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
  argTypes: {
    title: { control: "text" },
    description: { control: "text" },
    muted: { control: "boolean" },
  },
};
export default meta;
type Story = StoryObj<typeof SettingsRow>;

export const Default: Story = {
  args: { title: "Тёмная тема", description: "Использовать тёмную палитру интерфейса." },
  render: (args) => ({
    components: { SettingsRow, Toggle },
    setup: () => ({ args }),
    data: () => ({ value: true }),
    template: `
      <div style="width: 480px; border: 1px solid var(--border); border-radius: 8px;">
        <SettingsRow v-bind="args">
          <template #control>
            <Toggle v-model="value" />
          </template>
        </SettingsRow>
      </div>
    `,
  }),
};

export const WithoutDescription: Story = {
  args: { title: "Звуки уведомлений" },
  render: Default.render,
};

export const Muted: Story = {
  args: {
    title: "Облачная синхронизация",
    description: "Требуется подписка для активации.",
    muted: true,
  },
  render: Default.render,
};

export const Group: Story = {
  render: () => ({
    components: { SettingsRow, Toggle },
    data: () => ({ a: true, b: false, c: true }),
    template: `
      <div style="width: 480px; border: 1px solid var(--border); border-radius: 8px;">
        <SettingsRow title="Автосохранение" description="Сохранять заметки каждые 5 секунд">
          <template #control><Toggle v-model="a" /></template>
        </SettingsRow>
        <SettingsRow title="Глобальные хоткеи" description="Cmd+Space — открыть лаунчер">
          <template #control><Toggle v-model="b" /></template>
        </SettingsRow>
        <SettingsRow title="Запуск при старте системы">
          <template #control><Toggle v-model="c" /></template>
        </SettingsRow>
      </div>
    `,
  }),
};
