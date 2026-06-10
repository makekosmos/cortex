// Pattern: Titlebar в контексте реального desktop-окна. Показывает как
// `<Titlebar>` сочетается с `<DesktopChrome>` + `<DesktopContentSurface>`
// и сайдбаром — как Eden / Delphi / Dashboard в реальности.
//
// Не «изолированный» story компонента, а composed example для пощупать
// финальный layout.

import type { Meta, StoryObj } from "@storybook/vue3-vite";
import DesktopChrome from "./DesktopChrome.vue";
import DesktopContentSurface from "./DesktopContentSurface.vue";
import Titlebar from "./Titlebar.vue";
import TitlebarHistoryControls from "./TitlebarHistoryControls.vue";

const meta: Meta = {
  title: "Patterns/Titlebar in window",
  tags: ["autodocs"],
  parameters: {
    layout: "fullscreen",
  },
};
export default meta;
type Story = StoryObj;

export const EdenStyle: Story = {
  render: () => ({
    components: { DesktopChrome, DesktopContentSurface, Titlebar, TitlebarHistoryControls },
    template: `
      <div style="width: 100vw; height: 100vh;">
        <DesktopChrome platform="windows" title="Eden">
          <template #titlebar-leading>
            <TitlebarHistoryControls />
          </template>
          <template #titlebar-center>
            <span style="font-size: 13px; font-weight: 500;">Сегодняшняя заметка</span>
          </template>
          <template #titlebar-trailing>
            <button style="background: transparent; border: none; color: var(--muted-foreground); padding: 0 12px; font-size: 14px; cursor: pointer;">⚙</button>
          </template>
          <template #sidebar>
            <div style="width: 220px; padding: 16px; color: var(--sidebar-foreground); font-family: var(--font-sans);">
              <div style="font-weight: 600; margin-bottom: 12px; font-size: 13px; color: var(--muted-foreground); text-transform: uppercase; letter-spacing: 0.05em;">Заметки</div>
              <div style="display: flex; flex-direction: column; gap: 4px; font-size: 13px;">
                <div style="padding: 6px 10px; border-radius: 6px; background: color-mix(in srgb, var(--accent) 12%, transparent); color: var(--accent);">Сегодняшняя заметка</div>
                <div style="padding: 6px 10px; border-radius: 6px;">Идея фичи</div>
                <div style="padding: 6px 10px; border-radius: 6px;">Долгий перерыв</div>
                <div style="padding: 6px 10px; border-radius: 6px;">Скетч</div>
              </div>
            </div>
          </template>
          <DesktopContentSurface>
            <div style="max-width: 640px; margin: 0 auto; font-family: var(--font-sans);">
              <h1 style="margin-top: 0; font-size: 28px; font-weight: 700; letter-spacing: -0.02em;">Сегодняшняя заметка</h1>
              <p style="color: var(--muted-foreground); font-size: 13px; margin-bottom: 24px;">2026-05-18 · черновик</p>
              <p style="font-size: 15px; line-height: 1.6;">Это пример того, как Titlebar сочетается с DesktopChrome + sidebar + content surface в реальном Eden окне. Видно левый-верхний скруглённый угол, переход к sidebar'у, native кнопки управления окном справа.</p>
              <p style="font-size: 15px; line-height: 1.6;">В изолированной story Titlebar смотрится как полоска без контекста — здесь видно как он реально интегрируется в frame окна.</p>
            </div>
          </DesktopContentSurface>
        </DesktopChrome>
      </div>
    `,
  }),
};

export const KeplerLauncherStyle: Story = {
  parameters: {
    layout: "centered",
    viewport: { defaultViewport: "launcher" },
  },
  render: () => ({
    components: { DesktopChrome, DesktopContentSurface, Titlebar },
    template: `
      <div style="width: 720px; height: 460px; border-radius: 12px; overflow: hidden; box-shadow: 0 20px 60px rgba(0,0,0,0.5);">
        <DesktopChrome platform="windows">
          <template #titlebar-leading>
            <span style="font-size: 12px; color: var(--muted-foreground); padding-left: 12px;">Kepler</span>
          </template>
          <DesktopContentSurface>
            <div style="padding: 24px; font-family: var(--font-sans); color: var(--foreground);">
              <input type="text" placeholder="Поиск команд..." style="width: 100%; padding: 12px 16px; font-size: 16px; background: transparent; border: 1px solid var(--border); border-radius: 8px; color: var(--foreground); outline: none;" />
              <div style="margin-top: 16px; font-size: 12px; color: var(--muted-foreground);">Раскадровка launcher с titlebar (720×460).</div>
            </div>
          </DesktopContentSurface>
        </DesktopChrome>
      </div>
    `,
  }),
};
