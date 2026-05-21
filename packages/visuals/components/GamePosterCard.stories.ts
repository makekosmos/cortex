import type { Meta, StoryObj } from "@storybook/vue3-vite";
import GamePosterCard from "./GamePosterCard.vue";

const meta: Meta<typeof GamePosterCard> = {
  title: "Display/GamePosterCard",
  component: GamePosterCard,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
  argTypes: {
    to: { control: "text" },
    title: { control: "text" },
    eyebrow: { control: "text" },
    coverSrc: { control: "text" },
  },
};
export default meta;
type Story = StoryObj<typeof GamePosterCard>;

export const WithCover: Story = {
  args: {
    to: "#hades",
    title: "Hades II",
    eyebrow: "Roguelike",
    coverSrc: "https://images.unsplash.com/photo-1542751371-adc38448a05e?w=400&h=560&fit=crop",
  },
  decorators: [
    (Story) => ({ components: { Story }, template: `<div style="width: 220px;"><Story /></div>` }),
  ],
};

export const Placeholder: Story = {
  args: { to: "#game", title: "Без обложки", eyebrow: "Indie" },
  decorators: [
    (Story) => ({ components: { Story }, template: `<div style="width: 220px;"><Story /></div>` }),
  ],
  render: (args) => ({
    components: { GamePosterCard },
    setup: () => ({ args }),
    template: `
      <GamePosterCard v-bind="args">
        <template #placeholder>
          <span style="font-size: 24px; opacity: 0.4;">🎮</span>
        </template>
      </GamePosterCard>
    `,
  }),
};

export const Grid: Story = {
  render: () => ({
    components: { GamePosterCard },
    template: `
      <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 1rem; width: 720px;">
        <GamePosterCard to="#1" title="Hollow Knight" eyebrow="Metroidvania" />
        <GamePosterCard to="#2" title="Celeste" eyebrow="Platformer" />
        <GamePosterCard to="#3" title="Outer Wilds" eyebrow="Exploration" />
      </div>
    `,
  }),
};
