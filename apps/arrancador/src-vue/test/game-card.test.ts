import { mount } from "@vue/test-utils";
import GameCard from "@vue-app/components/GameCard.vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { createTestGame } from "@/types";

describe("GameCard", () => {
  it("renders the shared poster card contract for the active Vue renderer", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/", component: { template: "<div />" } },
        { path: "/game/:id", component: { template: "<div />" } },
      ],
    });
    await router.push("/");
    await router.isReady();

    const wrapper = mount(GameCard, {
      props: {
        game: createTestGame({
          id: "poster-game",
          name: "Valorant",
          genres: "Shooter, Action",
          background_image: "https://cdn.example.com/valorant.jpg",
        }),
      },
      global: {
        plugins: [router as never],
      },
    });

    const link = wrapper.get("a");
    expect(link.attributes("href")).toBe("/game/poster-game");
    expect(wrapper.text()).toContain("Valorant");
    expect(wrapper.text()).toContain("Шутер");
    expect(wrapper.get("img").attributes("src")).toBe("https://cdn.example.com/valorant.jpg");
  });
});
