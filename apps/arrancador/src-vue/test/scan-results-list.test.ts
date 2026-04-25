import { mount } from "@vue/test-utils";
import ScanResultsList from "@vue-app/components/scan/ScanResultsList.vue";

const scanResult = {
  path: "C:\\Games\\Control\\Control.exe",
  file_name: "Control.exe",
  selected: true,
  alreadyAdded: false,
  customName: "Control",
  cpuUsage: 12.5,
  gpuUsage: 0,
};

describe("ScanResultsList", () => {
  it("renders scan results and emits list actions", async () => {
    const wrapper = mount(ScanResultsList, {
      props: {
        activeTab: "processes" as const,
        currentListLength: 1,
        filteredResults: [scanResult],
        selectedCount: 1,
        newCount: 1,
        loadingProcesses: false,
        adding: false,
        error: null,
        filter: "",
        sortBy: "cpu" as const,
        "onUpdate:filter": (value: string) => wrapper.setProps({ filter: value }),
        "onUpdate:sortBy": (value: "name" | "cpu") =>
          wrapper.setProps({ sortBy: value }),
      },
    });

    expect(wrapper.text()).toContain("Control");
    expect(wrapper.text()).toContain("12.5%");

    await wrapper.get("input[placeholder]").setValue("con");
    expect(wrapper.emitted("update:filter")?.[0]).toEqual(["con"]);

    await wrapper.get("input[data-testid='scan-entry-name']").setValue("Control Ultimate");
    expect(wrapper.emitted("updateName")?.[0]).toEqual([
      "processes",
      scanResult.path,
      "Control Ultimate",
    ]);

    await wrapper.get("button[title]").trigger("click");
    expect(wrapper.emitted("refreshUsage")).toHaveLength(1);

    await wrapper.findAll("button")[3]?.trigger("click");
    expect(wrapper.emitted("selectAll")?.[0]).toEqual(["processes"]);

    await wrapper.findAll("button")[4]?.trigger("click");
    expect(wrapper.emitted("deselectAll")?.[0]).toEqual(["processes"]);

    await wrapper.findAll("button")[5]?.trigger("click");
    expect(wrapper.emitted("toggleSelect")?.[0]).toEqual([
      "processes",
      scanResult.path,
    ]);

    const buttonsAfterActions = wrapper.findAll("button");
    await buttonsAfterActions[buttonsAfterActions.length - 1]?.trigger("click");
    expect(wrapper.emitted("addSelected")?.[0]).toEqual(["processes"]);
  });

  it("emits retry from the error state", async () => {
    const wrapper = mount(ScanResultsList, {
      props: {
        activeTab: "processes" as const,
        currentListLength: 0,
        filteredResults: [],
        selectedCount: 0,
        newCount: 0,
        loadingProcesses: false,
        adding: false,
        error: "GPU query failed",
        filter: "",
        sortBy: "cpu" as const,
      },
    });

    expect(wrapper.text()).toContain("GPU query failed");
    await wrapper.get("button").trigger("click");
    expect(wrapper.emitted("loadProcesses")).toHaveLength(1);
  });
});
