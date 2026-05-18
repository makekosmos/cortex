import type { Meta, StoryObj } from "@storybook/vue3-vite";
import TitlebarHistoryControls from "./TitlebarHistoryControls.vue";

const meta: Meta<typeof TitlebarHistoryControls> = {
  title: "Window/TitlebarHistoryControls",
  component: TitlebarHistoryControls,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
  argTypes: {
    backDisabled: { control: "boolean" },
    forwardDisabled: { control: "boolean" },
    backTitle: { control: "text" },
    forwardTitle: { control: "text" },
  },
};
export default meta;
type Story = StoryObj<typeof TitlebarHistoryControls>;

export const Default: Story = {
  args: {},
};

export const BackDisabled: Story = {
  args: { backDisabled: true },
};

export const BothDisabled: Story = {
  args: { backDisabled: true, forwardDisabled: true },
};
