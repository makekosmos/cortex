import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import { computed, nextTick, shallowRef } from "vue";
import type { UsageProcessCandidate } from "../../src/types";

const pickerItems = shallowRef<UsageProcessCandidate[]>([]);
const pickerLoading = shallowRef(false);
const pickerError = shallowRef<string | null>(null);
const pickerSelected = shallowRef<UsageProcessCandidate[]>([]);
const pickerQuery = shallowRef("");

vi.mock("@vue-app/composables/useUsageProcessPicker", () => ({
  useUsageProcessPicker: () => ({
    query: pickerQuery,
    items: pickerItems,
    loading: pickerLoading,
    error: pickerError,
    selectedBindings: computed(() =>
      pickerSelected.value.map((item) => ({
        match_type: item.binding_match_type,
        match_value: item.binding_match_value,
      })),
    ),
    selectedCount: computed(() => pickerSelected.value.length),
    isSelected: (trackedAppId: string) =>
      pickerSelected.value.some((item) => item.tracked_app_id === trackedAppId),
    toggleSelection: (item: UsageProcessCandidate) => {
      pickerSelected.value = pickerSelected.value.some(
        (selected) => selected.tracked_app_id === item.tracked_app_id,
      )
        ? pickerSelected.value.filter(
            (selected) => selected.tracked_app_id !== item.tracked_app_id,
          )
        : [...pickerSelected.value, item];
    },
  }),
}));

import GameProcessBindingPickerModal from "@vue-app/components/game-detail/GameProcessBindingPickerModal.vue";
import GameProcessBindingsSection from "@vue-app/components/game-detail/GameProcessBindingsSection.vue";

const candidate: UsageProcessCandidate = {
  tracked_app_id: "tracked-1",
  display_name: "Control",
  process_name: "Control.exe",
  exe_path: "C:\\Games\\Control\\Control.exe",
  binding_match_type: "exe_path",
  binding_match_value: "C:\\Games\\Control\\Control.exe",
  binding_normalized_value: "c:\\games\\control\\control.exe",
  session_count: 2,
  last_seen_at: "2026-04-24T10:00:00.000Z",
};

async function clickBodySelector(selector: string) {
  const element = document.body.querySelector(selector);
  expect(element).toBeTruthy();
  element?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  await nextTick();
}

afterEach(() => {
  document.body.innerHTML = "";
  pickerItems.value = [];
  pickerLoading.value = false;
  pickerError.value = null;
  pickerSelected.value = [];
  pickerQuery.value = "";
});

describe("game process binding components", () => {
  it("renders primary and explicit bindings and emits remove actions", async () => {
    const wrapper = mount(GameProcessBindingsSection, {
      props: {
        primaryExePath: "C:\\Games\\Control\\Control.exe",
        bindings: [
          {
            id: 7,
            game_id: "game-1",
            match_type: "process_name",
            match_value: "Control_DX12.exe",
            created_at: "2026-04-24T00:00:00.000Z",
          },
        ],
        adding: false,
        removingBindingId: null,
      },
    });

    expect(wrapper.text()).toContain("primary");
    expect(wrapper.text()).toContain("Control_DX12.exe");

    const buttons = wrapper.findAll("button");
    await buttons[1]?.trigger("click");

    expect(wrapper.emitted("removeBinding")?.[0]).toEqual([7]);
  });

  it("submits selected usage tracker bindings from the picker modal", async () => {
    pickerItems.value = [candidate];

    const wrapper = mount(GameProcessBindingPickerModal, {
      props: {
        open: true,
        existingBindingKeys: [],
        submitting: false,
        "onUpdate:open": (value: boolean) => wrapper.setProps({ open: value }),
      },
      attachTo: document.body,
    });

    await clickBodySelector("[data-testid='usage-process-option-tracked-1']");
    await clickBodySelector("[data-testid='usage-process-submit']");

    expect(wrapper.emitted("submit")?.[0]).toEqual([
      [
        {
          match_type: "exe_path",
          match_value: "C:\\Games\\Control\\Control.exe",
        },
      ],
    ]);
  });

  it("blocks already-bound usage tracker candidates", async () => {
    pickerItems.value = [candidate];

    mount(GameProcessBindingPickerModal, {
      props: {
        open: true,
        existingBindingKeys: [candidate.binding_normalized_value],
        submitting: false,
      },
      attachTo: document.body,
    });

    const option = document.body.querySelector(
      "[data-testid='usage-process-option-tracked-1']",
    );
    const submit = document.body.querySelector("[data-testid='usage-process-submit']");

    expect(option?.getAttribute("disabled")).toBeDefined();
    expect(submit?.getAttribute("disabled")).toBeDefined();
  });
});
