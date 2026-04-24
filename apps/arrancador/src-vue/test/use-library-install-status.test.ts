import { flushPromises, mount } from "@vue/test-utils";
import {
  type LibraryInstallGame,
  mapWithConcurrency,
  useLibraryInstallStatus,
} from "@vue-app/composables/useLibraryInstallStatus";
import { defineComponent, nextTick, shallowRef } from "vue";

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((innerResolve) => {
    resolve = innerResolve;
  });
  return { promise, resolve };
}

async function flushUi() {
  await flushPromises();
  await nextTick();
  await flushPromises();
  await nextTick();
}

function mountHarness(
  initialGames: LibraryInstallGame[],
  checkInstalled: (id: string) => Promise<boolean>,
) {
  const Harness = defineComponent({
    setup() {
      const games = shallowRef(initialGames);
      const installStatus = useLibraryInstallStatus({
        games: () => games.value,
        checkInstalled,
        concurrency: 2,
        log: { error: vi.fn() },
      });

      return {
        games,
        ...installStatus,
      };
    },
    template: "<div />",
  });

  return mount(Harness);
}

describe("useLibraryInstallStatus", () => {
  it("reuses cached status when game identity and path are unchanged", async () => {
    const checkInstalled = vi.fn(async () => true);
    const wrapper = mountHarness(
      [{ id: "game-1", exe_path: "C:\\Games\\One\\One.exe" }],
      checkInstalled,
    );
    const vm = wrapper.vm as unknown as {
      games: LibraryInstallGame[];
      installedById: Record<string, boolean>;
    };

    await flushUi();
    expect(checkInstalled).toHaveBeenCalledTimes(1);
    expect(vm.installedById).toEqual({ "game-1": true });

    vm.games = [{ id: "game-1", exe_path: "C:\\Games\\One\\One.exe" }];
    await flushUi();

    expect(checkInstalled).toHaveBeenCalledTimes(1);
    expect(vm.installedById).toEqual({ "game-1": true });
  });

  it("does not let stale async checks overwrite newer game lists", async () => {
    const slow = createDeferred<boolean>();
    const fast = createDeferred<boolean>();
    const checkInstalled = vi.fn((id: string) =>
      id === "slow-game" ? slow.promise : fast.promise,
    );
    const wrapper = mountHarness(
      [{ id: "slow-game", exe_path: "C:\\Games\\Slow\\Slow.exe" }],
      checkInstalled,
    );
    const vm = wrapper.vm as unknown as {
      games: LibraryInstallGame[];
      installedById: Record<string, boolean>;
    };

    await nextTick();
    vm.games = [{ id: "fast-game", exe_path: "C:\\Games\\Fast\\Fast.exe" }];
    await nextTick();

    fast.resolve(false);
    await flushUi();
    expect(vm.installedById).toEqual({ "fast-game": false });

    slow.resolve(true);
    await flushUi();
    expect(vm.installedById).toEqual({ "fast-game": false });
  });

  it("limits concurrent install checks", async () => {
    let active = 0;
    let maxActive = 0;

    const result = await mapWithConcurrency([1, 2, 3, 4, 5, 6], 2, async (item) => {
      active += 1;
      maxActive = Math.max(maxActive, active);
      await new Promise((resolve) => setTimeout(resolve, 0));
      active -= 1;
      return item * 2;
    });

    expect(result).toEqual([2, 4, 6, 8, 10, 12]);
    expect(maxActive).toBeLessThanOrEqual(2);
  });
});
