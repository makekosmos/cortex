import { mount } from "@vue/test-utils";
import ArkSettingsSection from "@vue-app/components/settings/ArkSettingsSection.vue";
import type { ArkConnectionInfo, GamesArkSyncResult } from "@/types";

describe("ArkSettingsSection", () => {
  const arkConnection: ArkConnectionInfo = {
    ark_db_path: "D:/Vaults/Main/.ark/ark.db",
    ark_db_exists: true,
    ark_db_directory: "D:/Vaults/Main/.ark",
    uses_shared_selection: true,
    space_code: "main-space",
    space_id: "space-1",
    selection_source: "shared-selection",
    vault_path: "D:/Vaults/Main",
  };

  const syncResult: GamesArkSyncResult = {
    total: 17,
    synced: 17,
    failed: 0,
  };

  it("renders the current Ark connection and emits actions", async () => {
    const wrapper = mount(ArkSettingsSection, {
      props: {
        sectionId: "settings-ark",
        arkConnection,
        syncPending: false,
        syncFeedback: {
          tone: "success",
          text: "Игры синхронизированы с текущей Ark-базой.",
        },
        syncResult,
      },
    });

    expect(wrapper.text()).toContain("Текущее подключение к Ark");
    expect(wrapper.text()).toContain("main-space");
    expect(wrapper.text()).toContain("D:/Vaults/Main/.ark/ark.db");
    expect(wrapper.text()).toContain("Синхронизация завершена: 17 из 17");

    const buttons = wrapper.findAll("button");
    await buttons[0]?.trigger("click");
    await buttons[1]?.trigger("click");
    await buttons[2]?.trigger("click");

    expect(wrapper.emitted("openDatabase")).toHaveLength(1);
    expect(wrapper.emitted("openDirectory")).toHaveLength(1);
    expect(wrapper.emitted("syncGames")).toHaveLength(1);
  });
});
