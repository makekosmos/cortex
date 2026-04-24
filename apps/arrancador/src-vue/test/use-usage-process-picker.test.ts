import { flushPromises, mount } from "@vue/test-utils";
import { useUsageProcessPicker } from "@vue-app/composables/useUsageProcessPicker";
import { defineComponent, nextTick } from "vue";
import type { NewGameProcessBinding, UsageProcessCandidate } from "@/types";

const { gamesApiMock } = vi.hoisted(() => ({
  gamesApiMock: {
    listRecentUsageProcesses: vi.fn(),
    searchUsageProcesses: vi.fn(),
  },
}));

vi.mock("@/lib/api", () => ({
  gamesApi: gamesApiMock,
}));

const recentProcesses: UsageProcessCandidate[] = [
  {
    tracked_app_id: "tracked-main",
    display_name: "Chronicle",
    exe_path: "C:\\Games\\Chronicle\\Chronicle.exe",
    process_name: "Chronicle.exe",
    last_seen_at: "2026-04-23T09:00:00.000Z",
    session_count: 4,
    binding_match_type: "exe_path",
    binding_match_value: "C:\\Games\\Chronicle\\Chronicle.exe",
    binding_normalized_value: "c:\\games\\chronicle\\chronicle.exe",
  },
];

const searchProcesses: UsageProcessCandidate[] = [
  {
    tracked_app_id: "tracked-helper",
    display_name: "Chronicle Helper",
    exe_path: null,
    process_name: "Chronicle Helper.exe",
    last_seen_at: "2026-04-23T10:00:00.000Z",
    session_count: 2,
    binding_match_type: "process_name",
    binding_match_value: "Chronicle Helper.exe",
    binding_normalized_value: "chronicle helper.exe",
  },
];

const PickerHarness = defineComponent({
  props: {
    open: {
      type: Boolean,
      required: true,
    },
  },
  setup(props) {
    return useUsageProcessPicker({
      isOpen: () => props.open,
    });
  },
  template: `
    <div>
      <div data-testid="selected-count">{{ selectedCount }}</div>
      <div data-testid="selected-bindings">{{ JSON.stringify(selectedBindings) }}</div>
    </div>
  `,
});

async function flushUi() {
  await flushPromises();
  await nextTick();
  await flushPromises();
  await nextTick();
}

describe("useUsageProcessPicker", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    gamesApiMock.listRecentUsageProcesses.mockResolvedValue(recentProcesses);
    gamesApiMock.searchUsageProcesses.mockResolvedValue(searchProcesses);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("loads recent processes, debounces search, and preserves selected bindings across result sets", async () => {
    const wrapper = mount(PickerHarness, {
      props: {
        open: true,
      },
    });
    const vm = wrapper.vm as unknown as {
      items: UsageProcessCandidate[];
      query: string;
      selectedCount: number;
      selectedBindings: NewGameProcessBinding[];
      toggleSelection: (candidate: UsageProcessCandidate) => void;
    };

    await flushUi();

    expect(gamesApiMock.listRecentUsageProcesses).toHaveBeenCalledTimes(1);
    expect(gamesApiMock.listRecentUsageProcesses).toHaveBeenCalledWith(10);
    expect(vm.items).toEqual(recentProcesses);

    const recentProcess = recentProcesses[0];
    if (!recentProcess) {
      throw new Error("Expected seeded recent process");
    }
    vm.toggleSelection(recentProcess);
    await flushUi();

    vm.query = "helper";
    await flushUi();
    await vi.advanceTimersByTimeAsync(249);
    expect(gamesApiMock.searchUsageProcesses).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    await flushUi();

    expect(gamesApiMock.searchUsageProcesses).toHaveBeenCalledTimes(1);
    expect(gamesApiMock.searchUsageProcesses).toHaveBeenCalledWith("helper", 10);
    expect(vm.items).toEqual(searchProcesses);

    const searchProcess = searchProcesses[0];
    if (!searchProcess) {
      throw new Error("Expected seeded search process");
    }
    vm.toggleSelection(searchProcess);
    await flushUi();

    expect(vm.selectedCount).toBe(2);
    expect(vm.selectedBindings).toEqual([
      {
        match_type: "exe_path",
        match_value: "C:\\Games\\Chronicle\\Chronicle.exe",
      },
      {
        match_type: "process_name",
        match_value: "Chronicle Helper.exe",
      },
    ]);
  });
});
