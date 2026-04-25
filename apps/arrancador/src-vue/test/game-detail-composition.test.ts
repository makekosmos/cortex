import { mount } from "@vue/test-utils";
import GameBackLink from "@vue-app/components/game-detail/GameBackLink.vue";
import GameDangerZone from "@vue-app/components/game-detail/GameDangerZone.vue";
import GameDetailBackupSection from "@vue-app/components/game-detail/GameDetailBackupSection.vue";
import GameDetailDialogs from "@vue-app/components/game-detail/GameDetailDialogs.vue";
import GameDetailMetadataSection from "@vue-app/components/game-detail/GameDetailMetadataSection.vue";
import GameDetailSections from "@vue-app/components/game-detail/GameDetailSections.vue";
import GameDetailStateSection from "@vue-app/components/game-detail/GameDetailStateSection.vue";
import GameEditDialog from "@vue-app/components/game-detail/GameEditDialog.vue";
import GameMetadataSearchModal from "@vue-app/components/game-detail/GameMetadataSearchModal.vue";
import GameMissingState from "@vue-app/components/game-detail/GameMissingState.vue";
import GameProcessBindingsSection from "@vue-app/components/game-detail/GameProcessBindingsSection.vue";
import GameRatingModal from "@vue-app/components/game-detail/GameRatingModal.vue";
import { nextTick } from "vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { type Backup, createTestGame, type RawgGame } from "@/types";

const backups: Backup[] = [
  {
    id: "backup-latest",
    game_id: "game-1",
    backup_path: "D:/Backups/latest",
    backup_size: 2048,
    created_at: "2026-04-23T10:00:00.000Z",
    is_auto: false,
    notes: null,
  },
  {
    id: "backup-older",
    game_id: "game-1",
    backup_path: "D:/Backups/older",
    backup_size: 1024,
    created_at: "2026-04-22T10:00:00.000Z",
    is_auto: true,
    notes: null,
  },
];

const rawgResult: RawgGame = {
  id: 42,
  name: "Control",
  slug: "control",
  released: "2019-08-27",
  background_image: "https://cdn.example.com/control.jpg",
  metacritic: 85,
  rating: 4.4,
  ratings_count: 1000,
  genres: null,
  platforms: null,
};

afterEach(() => {
  document.body.innerHTML = "";
});

function withTeleportTarget() {
  return {
    attachTo: document.body,
  };
}

async function clickBodyButton(index: number) {
  const button = document.body.querySelectorAll("button")[index];
  expect(button).toBeTruthy();
  button?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  await nextTick();
}

describe("game detail composition components", () => {
  it("renders route shell helpers with library navigation links", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: { template: "<div />" } }],
    });
    await router.push("/");
    await router.isReady();

    const backLink = mount(GameBackLink, {
      global: { plugins: [router as never] },
    });
    const missing = mount(GameMissingState, {
      global: { plugins: [router as never] },
    });

    expect(backLink.get("a").attributes("href")).toBe("/");
    expect(missing.text()).toContain("Игра не найдена");
    expect(missing.get("a").attributes("href")).toBe("/");
  });

  it("wires the game detail section layout without hiding child contracts", () => {
    const wrapper = mount(GameDetailSections, {
      props: {
        game: createTestGame({
          name: "Control",
          exe_path: "C:\\Games\\Control\\Control.exe",
        }),
        playStatus: "not_started",
        userRating: null,
        userNote: "",
        savePathDraft: "{PATHTOGAME}\\Saves",
        showAllBackups: false,
        savingRating: false,
        savingNote: false,
        addingProcessBindings: false,
        removingProcessBindingId: null,
        savePathPreviewParts: ["C:\\Games\\Control", "\\Saves"],
        backupEnabled: true,
        creatingBackup: false,
        savingPath: false,
        locatingSavePath: false,
        canOpenSavePath: true,
        backups,
        latestBackup: backups[0] ?? null,
        olderBackups: backups.slice(1),
        loadingBackups: false,
        restoring: false,
      },
    });

    const stateSection = wrapper.findComponent(GameDetailStateSection);
    const backupSection = wrapper.findComponent(GameDetailBackupSection);
    const processSection = wrapper.findComponent(GameProcessBindingsSection);

    stateSection.vm.$emit("update:playStatus", "completed");
    stateSection.vm.$emit("update:userRating", 8);
    stateSection.vm.$emit("update:userNote", "Updated");
    stateSection.vm.$emit("savePlayStatus");
    stateSection.vm.$emit("saveUserRating");
    stateSection.vm.$emit("openRatingModal");
    stateSection.vm.$emit("saveUserNote");
    wrapper.findComponent(GameDetailMetadataSection).vm.$emit("edit");
    wrapper.findComponent(GameDetailMetadataSection).vm.$emit("searchRawg");
    processSection.vm.$emit("addBindings", [
      { match_type: "process_name", match_value: "control.exe" },
    ]);
    processSection.vm.$emit("removeBinding", 7);
    backupSection.vm.$emit("update:savePathDraft", "D:\\Saves");
    backupSection.vm.$emit("update:showAllBackups", true);
    backupSection.vm.$emit("toggleBackupEnabled", false);
    backupSection.vm.$emit("createBackup");
    backupSection.vm.$emit("chooseSaveFolder");
    backupSection.vm.$emit("chooseSaveFile");
    backupSection.vm.$emit("insertGamePathToken");
    backupSection.vm.$emit("locateSavePath");
    backupSection.vm.$emit("openSavePath");
    backupSection.vm.$emit("saveGamePath");
    backupSection.vm.$emit("restoreBackup", "backup-latest");
    wrapper.findComponent(GameDangerZone).vm.$emit("delete");

    expect(wrapper.emitted("update:playStatus")?.[0]).toEqual(["completed"]);
    expect(wrapper.emitted("update:userRating")?.[0]).toEqual([8]);
    expect(wrapper.emitted("update:userNote")?.[0]).toEqual(["Updated"]);
    expect(wrapper.emitted("savePlayStatus")).toHaveLength(1);
    expect(wrapper.emitted("saveUserRating")).toHaveLength(1);
    expect(wrapper.emitted("openRatingModal")).toHaveLength(1);
    expect(wrapper.emitted("saveUserNote")).toHaveLength(1);
    expect(wrapper.emitted("edit")).toHaveLength(1);
    expect(wrapper.emitted("searchRawg")).toHaveLength(1);
    expect(wrapper.emitted("addProcessBindings")?.[0]).toEqual([
      [{ match_type: "process_name", match_value: "control.exe" }],
    ]);
    expect(wrapper.emitted("removeProcessBinding")?.[0]).toEqual([7]);
    expect(wrapper.emitted("update:savePathDraft")?.[0]).toEqual(["D:\\Saves"]);
    expect(wrapper.emitted("update:showAllBackups")?.[0]).toEqual([true]);
    expect(wrapper.emitted("toggleBackupEnabled")?.[0]).toEqual([false]);
    expect(wrapper.emitted("createBackup")).toHaveLength(1);
    expect(wrapper.emitted("chooseSaveFolder")).toHaveLength(1);
    expect(wrapper.emitted("chooseSaveFile")).toHaveLength(1);
    expect(wrapper.emitted("insertGamePathToken")).toHaveLength(1);
    expect(wrapper.emitted("locateSavePath")).toHaveLength(1);
    expect(wrapper.emitted("openSavePath")).toHaveLength(1);
    expect(wrapper.emitted("saveGamePath")).toHaveLength(1);
    expect(wrapper.emitted("restoreBackup")?.[0]).toEqual(["backup-latest"]);
    expect(wrapper.emitted("delete")).toHaveLength(1);
  });

  it("keeps dangerous actions and dialog stack contracts explicit", async () => {
    const danger = mount(GameDangerZone);
    await danger.get("button").trigger("click");
    expect(danger.emitted("delete")).toHaveLength(1);

    const dialogs = mount(GameDetailDialogs, {
      props: {
        descriptionOpen: true,
        ratingOpen: false,
        ratingDraft: 4,
        editOpen: true,
        editName: "Control",
        editDescription: "",
        editBackgroundImage: "",
        editCoverImage: "",
        metadataOpen: false,
        metadataQuery: "Control",
        metadataRename: false,
        description: "A brutalist action game.",
        gameName: "Control",
        savingEdit: false,
        metadataResults: [],
        searchingMetadata: false,
        applyingMetadata: false,
        backupProgress: {
          active: false,
          stage: "idle",
          message: "",
          done: 0,
          total: 0,
        },
      },
      ...withTeleportTarget(),
    });

    expect(document.body.textContent).toContain("A brutalist action game.");
    await clickBodyButton(0);
    expect(dialogs.emitted("update:descriptionOpen")?.[0]).toEqual([false]);

    dialogs.findComponent(GameRatingModal).vm.$emit("update:open", true);
    dialogs.findComponent(GameRatingModal).vm.$emit("update:ratingDraft", 7);
    dialogs.findComponent(GameRatingModal).vm.$emit("save", 7);
    dialogs.findComponent(GameEditDialog).vm.$emit("update:name", "Control Ultimate");
    dialogs.findComponent(GameEditDialog).vm.$emit("update:description", "Updated");
    dialogs
      .findComponent(GameEditDialog)
      .vm.$emit("update:backgroundImage", "https://cdn.example.com/bg.jpg");
    dialogs
      .findComponent(GameEditDialog)
      .vm.$emit("update:coverImage", "https://cdn.example.com/cover.jpg");
    dialogs.findComponent(GameEditDialog).vm.$emit("close");
    dialogs.findComponent(GameEditDialog).vm.$emit("save");
    dialogs.findComponent(GameEditDialog).vm.$emit("searchImage", {
      query: "Control",
      target: "background",
    });
    dialogs.findComponent(GameMetadataSearchModal).vm.$emit("update:open", true);
    dialogs.findComponent(GameMetadataSearchModal).vm.$emit("update:query", "Alan Wake");
    dialogs.findComponent(GameMetadataSearchModal).vm.$emit("update:rename", true);
    dialogs.findComponent(GameMetadataSearchModal).vm.$emit("search");
    dialogs.findComponent(GameMetadataSearchModal).vm.$emit("apply", rawgResult);

    expect(dialogs.emitted("update:ratingOpen")?.[0]).toEqual([true]);
    expect(dialogs.emitted("update:ratingDraft")?.[0]).toEqual([7]);
    expect(dialogs.emitted("saveRating")?.[0]).toEqual([7]);
    expect(dialogs.emitted("update:editName")?.[0]).toEqual(["Control Ultimate"]);
    expect(dialogs.emitted("update:editDescription")?.[0]).toEqual(["Updated"]);
    expect(dialogs.emitted("update:editBackgroundImage")?.[0]).toEqual([
      "https://cdn.example.com/bg.jpg",
    ]);
    expect(dialogs.emitted("update:editCoverImage")?.[0]).toEqual([
      "https://cdn.example.com/cover.jpg",
    ]);
    expect(dialogs.emitted("update:editOpen")?.[0]).toEqual([false]);
    expect(dialogs.emitted("saveEdit")).toHaveLength(1);
    expect(dialogs.emitted("searchImage")?.[0]).toEqual([
      { query: "Control", target: "background" },
    ]);
    expect(dialogs.emitted("update:metadataOpen")?.[0]).toEqual([true]);
    expect(dialogs.emitted("update:metadataQuery")?.[0]).toEqual(["Alan Wake"]);
    expect(dialogs.emitted("update:metadataRename")?.[0]).toEqual([true]);
    expect(dialogs.emitted("searchMetadata")).toHaveLength(1);
    expect(dialogs.emitted("applyMetadata")?.[0]).toEqual([rawgResult]);
  });
});
