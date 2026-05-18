import type { Meta, StoryObj } from "@storybook/vue3-vite";
import DesktopContentSurface from "./DesktopContentSurface.vue";

const meta: Meta<typeof DesktopContentSurface> = {
  title: "Window/DesktopContentSurface",
  component: DesktopContentSurface,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
  argTypes: {
    paddingTop: { control: "text" },
    paddingInline: { control: "text" },
    paddingBottom: { control: "text" },
    radiusTopLeft: { control: "text" },
    showLeftBorder: { control: "boolean" },
    scrollable: { control: "boolean" },
  },
  decorators: [
    (Story) => ({
      components: { Story },
      template: `<div style="width: 720px; height: 460px; background: var(--sidebar-bg, #1a1a1a); display: flex;"><Story /></div>`,
    }),
  ],
};
export default meta;
type Story = StoryObj<typeof DesktopContentSurface>;

export const Default: Story = {
  render: (args) => ({
    components: { DesktopContentSurface },
    setup: () => ({ args }),
    template: `
      <DesktopContentSurface v-bind="args">
        <h2 style="margin-top: 0;">Content surface</h2>
        <p>По умолчанию вне DesktopChrome surface считает что сайдбар есть → радиус 16px + левая граница.</p>
      </DesktopContentSurface>
    `,
  }),
};

export const NoLeftBorder: Story = {
  args: { showLeftBorder: false, radiusTopLeft: "0" },
  render: Default.render,
};

export const Scrollable: Story = {
  args: { scrollable: true },
  render: (args) => ({
    components: { DesktopContentSurface },
    setup: () => ({ args }),
    template: `
      <DesktopContentSurface v-bind="args">
        <div v-for="i in 40" :key="i" style="padding: 0.5rem 0; border-bottom: 1px solid var(--border);">
          Row #{{ i }}
        </div>
      </DesktopContentSurface>
    `,
  }),
};

export const CustomPadding: Story = {
  args: { paddingTop: "2rem", paddingInline: "2rem", paddingBottom: "2rem" },
  render: Default.render,
};
