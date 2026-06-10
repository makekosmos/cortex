import type { Meta, StoryObj } from "@storybook/vue3-vite";
import BlocklistCard from "./BlocklistCard.vue";

const meta: Meta<typeof BlocklistCard> = {
  title: "Display/BlocklistCard",
  component: BlocklistCard,
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
  },
  argTypes: {
    name: { control: "text" },
    icon: { control: "text" },
    preset: { control: "boolean" },
    active: { control: "boolean" },
  },
};
export default meta;
type Story = StoryObj<typeof BlocklistCard>;

const sampleDomains = [
  "tiktok.com",
  "www.tiktok.com",
  "twitter.com",
  "www.twitter.com",
  "reddit.com",
  "www.reddit.com",
  "youtube.com",
  "www.youtube.com",
];

export const Default: Story = {
  args: { name: "Distractions", domains: sampleDomains, icon: "🛡️" },
};

export const Preset: Story = {
  args: { name: "Distractions", domains: sampleDomains, icon: "🛡️", preset: true },
};

export const Active: Story = {
  args: { name: "Coding focus", domains: sampleDomains, icon: "💻", active: true },
};

export const Many: Story = {
  args: {
    name: "Brain rot",
    domains: Array.from({ length: 30 }, (_, i) => `domain${i}.com`),
    icon: "🧠",
    count: 30,
  },
};

export const Empty: Story = {
  args: { name: "Empty list", domains: [], icon: "📭" },
};

export const NoIcon: Story = {
  args: { name: "Custom list", domains: sampleDomains.slice(0, 3) },
};
