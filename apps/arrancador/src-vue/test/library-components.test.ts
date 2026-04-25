import { mount } from "@vue/test-utils";
import LibraryFiltersPanel from "@vue-app/components/library/LibraryFiltersPanel.vue";
import LibraryResults from "@vue-app/components/library/LibraryResults.vue";
import LibraryToolbar from "@vue-app/components/library/LibraryToolbar.vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { createTestGame } from "@/types";

const sortByLabel = {
  name: "По имени",
  lastPlayed: "Недавно запущенные",
  dateAdded: "Недавно добавленные",
  playCount: "По запускам",
  playtime: "По времени",
} as const;

const playedStateLabel = {
  all: "Все",
  played: "Играли",
  unplayed: "Не играли",
} as const;

const installStateLabel = {
  all: "Все",
  installed: "Установленные",
  not_installed: "Не установленные",
} as const;

const playStatusLabel = {
  all: "Все",
  not_started: "Не начато",
  in_progress: "В процессе",
  completed: "Пройдено",
  abandoned: "Брошено",
} as const;

async function createRouterPlugin() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: { template: "<div />" } },
      { path: "/scan", component: { template: "<div />" } },
      { path: "/game/:id", component: { template: "<div />" } },
    ],
  });
  await router.push("/");
  await router.isReady();
  return router;
}

function lastEvent(events: unknown[][] | undefined): unknown[] | undefined {
  return events ? events[events.length - 1] : undefined;
}

describe("library components", () => {
  it("emits toolbar model updates for search, view, filters, and sorting", async () => {
    const wrapper = mount(LibraryToolbar, {
      props: {
        totalGames: 3,
        favoriteCount: 1,
        activeFilterCount: 2,
        sortByLabel,
        searchQuery: "old",
        viewMode: "grid",
        sortBy: "name",
        showAdvancedFilters: false,
      },
    });

    await wrapper.get("input").setValue("valorant");
    await wrapper.findAll("button")[0]?.trigger("click");
    await wrapper.findAll("button")[2]?.trigger("click");
    await wrapper.get("select").setValue("playtime");

    expect(lastEvent(wrapper.emitted("update:searchQuery"))).toEqual(["valorant"]);
    expect(lastEvent(wrapper.emitted("update:showAdvancedFilters"))).toEqual([true]);
    expect(lastEvent(wrapper.emitted("update:viewMode"))).toEqual(["list"]);
    expect(lastEvent(wrapper.emitted("update:sortBy"))).toEqual(["playtime"]);
  });

  it("keeps advanced filter changes explicit through v-model events", async () => {
    const wrapper = mount(LibraryFiltersPanel, {
      props: {
        genreOptions: ["Action", "RPG"],
        platformOptions: ["PC", "Deck"],
        playedStateLabel,
        installStateLabel,
        playStatusLabel,
        translateGenre: (genre: string) => `ru:${genre}`,
        showFavoritesOnly: false,
        selectedGenres: [],
        selectedPlatforms: [],
        playedState: "all",
        installState: "all",
        playStatusState: "all",
        ratingMode: "user",
        minRating: "",
        maxRating: "",
        minMetacritic: "",
        maxMetacritic: "",
        minPlaytimeHours: "",
        maxPlaytimeHours: "",
        requireMetadata: false,
      },
    });

    await wrapper.findAll("button")[0]?.trigger("click");
    await wrapper.findAll("button")[1]?.trigger("click");
    await wrapper.findAll("button")[2]?.trigger("click");
    await wrapper.findAll("select")[3]?.setValue("installed");

    expect(lastEvent(wrapper.emitted("update:showFavoritesOnly"))).toEqual([true]);
    expect(wrapper.emitted("clearFilters")).toHaveLength(1);
    expect(lastEvent(wrapper.emitted("update:requireMetadata"))).toEqual([true]);
    expect(lastEvent(wrapper.emitted("update:installState"))).toEqual(["installed"]);
  });

  it("renders library result states without changing route contracts", async () => {
    const router = await createRouterPlugin();
    const game = createTestGame({
      id: "valorant",
      name: "VALORANT",
      genres: "Shooter",
      total_playtime: 3900,
      last_played: "2026-04-22T12:00:00.000Z",
      metacritic: 80,
      is_favorite: true,
    });

    const wrapper = mount(LibraryResults, {
      props: {
        games: [game],
        totalGameCount: 1,
        viewMode: "list",
      },
      global: {
        plugins: [router as never],
      },
    });

    expect(wrapper.get("a").attributes("href")).toBe("/game/valorant");
    expect(wrapper.text()).toContain("VALORANT");
    expect(wrapper.text()).toContain("1 ч 5 мин");

    await wrapper.setProps({ games: [] });
    expect(wrapper.text()).toContain("Игры не найдены");

    await wrapper.setProps({ totalGameCount: 0 });
    expect(wrapper.text()).toContain("Сканировать");
  });
});
