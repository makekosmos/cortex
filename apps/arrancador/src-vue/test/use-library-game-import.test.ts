import { flushPromises, mount } from "@vue/test-utils";
import { useLibraryGameImport } from "@vue-app/composables/useLibraryGameImport";
import { defineComponent, nextTick, shallowRef } from "vue";
import { createTestGame, type Game, type NewGame } from "@/types";

async function flushUi() {
  await flushPromises();
  await nextTick();
  await flushPromises();
  await nextTick();
}

function mountHarness(options: {
  initialGames?: Game[];
  addGames?: (games: NewGame[]) => Promise<Game[]>;
  updateGame?: (id: string, updates: { exe_path: string }) => Promise<unknown>;
  existsByPath?: (path: string) => Promise<boolean>;
  resolveShortcutTarget?: (path: string) => Promise<string>;
  confirm?: (message: string) => boolean;
  notify?: (payload: { tone: "success" | "warning" | "error"; title: string; description?: string }) => void;
}) {
  const Harness = defineComponent({
    setup() {
      const games = shallowRef(options.initialGames ?? []);
      return useLibraryGameImport({
        games: () => games.value,
        addGames: options.addGames ?? vi.fn(async () => []),
        updateGame: options.updateGame ?? vi.fn(async () => undefined),
        existsByPath: options.existsByPath ?? vi.fn(async () => false),
        resolveShortcutTarget:
          options.resolveShortcutTarget ?? vi.fn(async (path: string) => path),
        confirm: options.confirm ?? vi.fn(() => false),
        notify: options.notify ?? vi.fn(),
        log: { error: vi.fn() },
      });
    },
    template: "<div />",
  });

  return mount(Harness);
}

describe("useLibraryGameImport", () => {
  it("adds only unique valid executables and queues metadata for created games", async () => {
    const added = createTestGame({
      id: "elden-ring",
      name: "Elden Ring",
      exe_path: "C:\\Games\\Elden Ring\\eldenring.exe",
      exe_name: "eldenring.exe",
    });
    const addGames = vi.fn(async () => [added]);
    const notify = vi.fn();
    const wrapper = mountHarness({
      addGames,
      notify,
      resolveShortcutTarget: vi.fn(async () => "C:\\Games\\Elden Ring\\eldenring.exe"),
    });
    const vm = wrapper.vm as unknown as {
      handleDroppedPaths: (paths: string[]) => Promise<void>;
      metadataQueue: Game[];
      currentMetadataGame: Game | null;
    };

    await vm.handleDroppedPaths([
      "C:\\Shortcuts\\eldenring.lnk",
      "C:\\Games\\Elden Ring\\eldenring.exe",
      "C:\\Games\\Elden Ring\\readme.txt",
    ]);
    await flushUi();

    expect(addGames).toHaveBeenCalledTimes(1);
    expect(addGames).toHaveBeenCalledWith([
      {
        name: "eldenring",
        exe_path: "C:\\Games\\Elden Ring\\eldenring.exe",
        exe_name: "eldenring.exe",
      },
    ]);
    expect(vm.metadataQueue).toEqual([added]);
    expect(vm.currentMetadataGame).toEqual(added);
    expect(notify).toHaveBeenCalledWith(
      expect.objectContaining({
        tone: "success",
      }),
    );
  });

  it("merges a dropped executable into a matching existing game when confirmed", async () => {
    const existing = createTestGame({
      id: "arcadia",
      name: "Arcadia",
      exe_path: "C:\\Old\\arcadia.exe",
      exe_name: "arcadia.exe",
    });
    const addGames = vi.fn(async () => []);
    const updateGame = vi.fn(async () => undefined);
    const notify = vi.fn();
    const wrapper = mountHarness({
      initialGames: [existing],
      addGames,
      updateGame,
      notify,
      confirm: vi.fn(() => true),
    });
    const vm = wrapper.vm as unknown as {
      handleDroppedPaths: (paths: string[]) => Promise<void>;
    };

    await vm.handleDroppedPaths(["C:\\Games\\Arcadia\\arcadia.exe"]);
    await flushUi();

    expect(updateGame).toHaveBeenCalledWith("arcadia", {
      exe_path: "C:\\Games\\Arcadia\\arcadia.exe",
    });
    expect(addGames).not.toHaveBeenCalled();
    expect(notify).toHaveBeenCalledWith(
      expect.objectContaining({
        tone: "success",
      }),
    );
  });
});
