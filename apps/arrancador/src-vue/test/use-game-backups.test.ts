import { flushPromises, mount } from "@vue/test-utils";
import { useGameBackups } from "@vue-app/composables/useGameBackups";
import { defineComponent, nextTick, shallowRef } from "vue";
import { type Backup, createTestGame, type Game } from "@/types";

const { backupApiMock, browserMock } = vi.hoisted(() => ({
  backupApiMock: {
    getForGame: vi.fn(),
    create: vi.fn(),
    restore: vi.fn(),
  },
  browserMock: {
    handlers: new Map<string, (payload: unknown) => void>(),
    subscribeAppEvent: vi.fn((event: string, callback: (payload: unknown) => void) => {
      browserMock.handlers.set(event, callback);
      return vi.fn();
    }),
  },
}));

vi.mock("@/lib/api", () => ({
  backupApi: backupApiMock,
}));

vi.mock("@/lib/browser", () => ({
  subscribeAppEvent: browserMock.subscribeAppEvent,
}));

async function flushUi() {
  await flushPromises();
  await nextTick();
  await flushPromises();
  await nextTick();
}

function mountHarness(game: Game | null) {
  const Harness = defineComponent({
    setup() {
      const currentGame = shallowRef(game);
      return {
        currentGame,
        ...useGameBackups({
          game: () => currentGame.value,
          refreshGames: vi.fn(async () => undefined),
          notify: vi.fn(),
          confirm: vi.fn(() => true),
          log: { error: vi.fn() },
        }),
      };
    },
    template: "<div />",
  });

  return mount(Harness);
}

describe("useGameBackups", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    browserMock.handlers.clear();
  });

  it("sorts loaded backups and tracks progress only for the active game", async () => {
    const older: Backup = {
      id: "old",
      game_id: "game-1",
      backup_path: "C:\\Backups\\old",
      backup_size: 10,
      created_at: "2026-04-20T10:00:00.000Z",
      is_auto: false,
      notes: null,
    };
    const newer: Backup = {
      ...older,
      id: "new",
      backup_path: "C:\\Backups\\new",
      created_at: "2026-04-22T10:00:00.000Z",
    };
    backupApiMock.getForGame.mockResolvedValue([older, newer]);

    const wrapper = mountHarness(createTestGame({ id: "game-1" }));
    const vm = wrapper.vm as unknown as {
      backups: Backup[];
      latestBackup: Backup | null;
      backupProgress: {
        active: boolean;
        stage: string;
        message: string;
        done: number;
        total: number;
      };
    };

    await flushUi();

    expect(vm.backups.map((backup) => backup.id)).toEqual(["new", "old"]);
    expect(vm.latestBackup?.id).toBe("new");

    browserMock.handlers.get("backup:progress")?.({
      game_id: "other-game",
      stage: "copy",
      message: "ignored",
      done: 1,
      total: 2,
    });
    expect(vm.backupProgress.active).toBe(false);

    browserMock.handlers.get("backup:progress")?.({
      game_id: "game-1",
      stage: "copy",
      message: "Copying",
      done: 1,
      total: 2,
    });

    expect(vm.backupProgress).toMatchObject({
      active: true,
      stage: "copy",
      message: "Copying",
      done: 1,
      total: 2,
    });
  });
});
