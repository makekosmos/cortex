import type { Meta, StoryObj } from "@storybook/vue3-vite";
import DesktopChrome from "./DesktopChrome.vue";
import DesktopContentSurface from "./DesktopContentSurface.vue";

const meta: Meta<typeof DesktopChrome> = {
  title: "Window/DesktopChrome",
  component: DesktopChrome,
  tags: ["autodocs"],
  argTypes: {
    platform: { control: "select", options: ["mac", "windows", "linux"] },
    title: { control: "text" },
  },
  parameters: { layout: "fullscreen" },
};
export default meta;
type Story = StoryObj<typeof DesktopChrome>;

export const WithSidebar: Story = {
  args: { platform: "windows", title: "Eden" },
  render: (args) => ({
    components: { DesktopChrome, DesktopContentSurface },
    setup: () => ({ args }),
    template: `
      <div style="width: 100vw; height: 100vh;">
        <DesktopChrome v-bind="args">
          <template #sidebar>
            <div style="width: 200px; padding: 1rem; color: var(--sidebar-foreground);">
              <div style="font-weight: 600; margin-bottom: 0.5rem;">Заметки</div>
              <div style="display: flex; flex-direction: column; gap: 0.25rem; font-size: 13px;">
                <div>Сегодня</div>
                <div>Вчера</div>
                <div>Прошлая неделя</div>
              </div>
            </div>
          </template>
          <DesktopContentSurface>
            <h2 style="margin-top: 0;">Сегодняшняя заметка</h2>
            <p>Контент content surface. Скруглённый верхний-левый угол + левая граница автоматически появляются, потому что слот sidebar заполнен.</p>
          </DesktopContentSurface>
        </DesktopChrome>
      </div>
    `,
  }),
};

export const WithoutSidebar: Story = {
  args: { platform: "windows", title: "Dashboard" },
  render: (args) => ({
    components: { DesktopChrome, DesktopContentSurface },
    setup: () => ({ args }),
    template: `
      <div style="width: 100vw; height: 100vh;">
        <DesktopChrome v-bind="args">
          <DesktopContentSurface>
            <h2 style="margin-top: 0;">Без сайдбара</h2>
            <p>Когда слот sidebar не передан, content surface теряет скругление и левую границу — плоский край.</p>
          </DesktopContentSurface>
        </DesktopChrome>
      </div>
    `,
  }),
};

export const MacPlatform: Story = {
  args: { platform: "mac", title: "Kepler" },
  render: WithSidebar.render,
};
