import { flushPromises, mount } from "@vue/test-utils";
import { useGameMetadataSearch } from "@vue-app/composables/useGameMetadataSearch";
import { defineComponent, nextTick } from "vue";
import { createTestGame, type RawgGame } from "@/types";

const { metadataApiMock, browserMock } = vi.hoisted(() => ({
  metadataApiMock: {
    search: vi.fn(),
    apply: vi.fn(),
  },
  browserMock: {
    openPath: vi.fn(),
  },
}));

vi.mock("@/lib/api", () => ({
  metadataApi: metadataApiMock,
}));

vi.mock("@/lib/browser", () => browserMock);

async function flushUi() {
  await flushPromises();
  await nextTick();
  await flushPromises();
  await nextTick();
}

const rawgResult: RawgGame = {
  id: 42,
  name: "Arcadia",
  slug: "arcadia",
  released: "2026-04-01",
  background_image: null,
  metacritic: 90,
  rating: 4.5,
  ratings_count: 100,
  genres: [],
  platforms: [],
};

describe("useGameMetadataSearch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    metadataApiMock.apply.mockResolvedValue(createTestGame());
  });

  it("applies metadata and resets search state", async () => {
    const refreshGames = vi.fn(async () => undefined);
    const Harness = defineComponent({
      setup() {
        return useGameMetadataSearch({
          game: () => createTestGame({ id: "game-1" }),
          updateGame: vi.fn(async () => undefined),
          refreshGames,
          log: { error: vi.fn() },
        });
      },
      template: "<div />",
    });
    const wrapper = mount(Harness);
    const vm = wrapper.vm as unknown as {
      showMetadataSearch: boolean;
      metadataQuery: string;
      metadataResults: RawgGame[];
      renameFromMetadata: boolean;
      applyMetadata: (game: RawgGame) => Promise<void>;
    };

    vm.showMetadataSearch = true;
    vm.metadataQuery = "Arcadia";
    vm.metadataResults = [rawgResult];
    vm.renameFromMetadata = true;

    await vm.applyMetadata(rawgResult);
    await flushUi();

    expect(metadataApiMock.apply).toHaveBeenCalledWith("game-1", 42, true);
    expect(refreshGames).toHaveBeenCalledTimes(1);
    expect(vm.showMetadataSearch).toBe(false);
    expect(vm.metadataQuery).toBe("");
    expect(vm.metadataResults).toEqual([]);
  });
});
