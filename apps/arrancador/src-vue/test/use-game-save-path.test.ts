import { flushPromises, mount } from "@vue/test-utils";
import { useGameSavePath } from "@vue-app/composables/useGameSavePath";
import { GAME_PATH_TOKEN } from "@vue-app/lib/gameDetailDisplay";
import { defineComponent, nextTick, shallowRef } from "vue";
import { createTestGame, type Game } from "@/types";

const { backupApiMock, browserMock } = vi.hoisted(() => ({
  backupApiMock: {
    findGameSavePaths: vi.fn(),
  },
  browserMock: {
    openPath: vi.fn(),
    pickDirectoryPath: vi.fn(),
    pickFilePath: vi.fn(),
  },
}));

vi.mock("@/lib/api", () => ({
  backupApi: backupApiMock,
}));

vi.mock("@/lib/browser", () => browserMock);

async function flushUi() {
  await flushPromises();
  await nextTick();
  await flushPromises();
  await nextTick();
}

function mountHarness(game: Game) {
  const updateGame = vi.fn(async () => undefined);
  const Harness = defineComponent({
    setup() {
      const currentGame = shallowRef(game);
      return {
        currentGame,
        updateGame,
        ...useGameSavePath({
          game: () => currentGame.value,
          updateGame,
          notify: vi.fn(),
          log: { error: vi.fn() },
        }),
      };
    },
    template: "<div />",
  });

  return mount(Harness);
}

describe("useGameSavePath", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("inserts the game path token once and saves empty paths as null", async () => {
    const wrapper = mountHarness(createTestGame({ save_path: "Saves" }));
    const vm = wrapper.vm as unknown as {
      savePathDraft: string;
      insertGamePathToken: () => void;
      saveGamePath: () => Promise<void>;
      updateGame: ReturnType<typeof vi.fn>;
    };

    await flushUi();
    expect(vm.savePathDraft).toBe("Saves");

    vm.insertGamePathToken();
    expect(vm.savePathDraft).toBe(`${GAME_PATH_TOKEN}\\Saves`);

    vm.insertGamePathToken();
    expect(vm.savePathDraft).toBe(`${GAME_PATH_TOKEN}\\Saves`);

    vm.savePathDraft = "   ";
    await vm.saveGamePath();
    await flushUi();

    expect(vm.updateGame).toHaveBeenCalledWith("game-1", {
      save_path: null,
    });
  });
});
