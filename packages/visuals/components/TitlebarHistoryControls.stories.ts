import type { Meta, StoryObj } from "@storybook/vue3";
import TitlebarHistoryControls from "./TitlebarHistoryControls.vue";

const meta: Meta<typeof TitlebarHistoryControls> = {
  title: "Components/TitlebarHistoryControls",
  component: TitlebarHistoryControls,
  tags: ["autodocs"],
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
