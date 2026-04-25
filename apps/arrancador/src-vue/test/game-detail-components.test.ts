import { mount } from "@vue/test-utils";
import BackupProgressToast from "@vue-app/components/game-detail/BackupProgressToast.vue";
import GameDescriptionModal from "@vue-app/components/game-detail/GameDescriptionModal.vue";
import GameDetailBackupSection from "@vue-app/components/game-detail/GameDetailBackupSection.vue";
import GameDetailHeroSection from "@vue-app/components/game-detail/GameDetailHeroSection.vue";
import GameDetailInfoSection from "@vue-app/components/game-detail/GameDetailInfoSection.vue";
import GameDetailMetadataSection from "@vue-app/components/game-detail/GameDetailMetadataSection.vue";
import GameDetailStateSection from "@vue-app/components/game-detail/GameDetailStateSection.vue";
import GameEditDialog from "@vue-app/components/game-detail/GameEditDialog.vue";
import GameMetadataSearchModal from "@vue-app/components/game-detail/GameMetadataSearchModal.vue";
import GameRatingModal from "@vue-app/components/game-detail/GameRatingModal.vue";
import { nextTick } from "vue";
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

async function setBodyInputValue(selector: string, value: string) {
  const input = document.body.querySelector(selector) as HTMLInputElement | null;
  expect(input).toBeTruthy();
  if (!input) return;
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  await nextTick();
}

async function setBodyCheckboxValue(selector: string, checked: boolean) {
  const input = document.body.querySelector(selector) as HTMLInputElement | null;
  expect(input).toBeTruthy();
  if (!input) return;
  input.checked = checked;
  input.dispatchEvent(new Event("change", { bubbles: true }));
  await nextTick();
}

describe("game detail components", () => {
  it("renders hero actions and emits launch/favorite/description events", async () => {
    const wrapper = mount(GameDetailHeroSection, {
      props: {
        game: createTestGame({
          name: "Control",
          is_favorite: false,
          background_image: "https://cdn.example.com/control.jpg",
        }),
        heroImage: "https://cdn.example.com/control.jpg",
        heroGenres: "Action",
        heroDescription: "A brutalist action game.",
        heroMeta: "2019 В· 2 С‡",
        launching: false,
        restoring: false,
        isMissing: false,
        runningCount: 0,
        checkingRunning: false,
      },
    });

    expect(wrapper.text()).toContain("Control");
    expect(wrapper.text()).toContain("Action");
    expect(wrapper.find("[data-testid='game-detail-hero-description']").text()).toContain(
      "brutalist",
    );

    const buttons = wrapper.findAll("button");
    await buttons[0]?.trigger("click");
    await buttons[1]?.trigger("click");
    await buttons[2]?.trigger("click");

    expect(wrapper.emitted("launch")).toHaveLength(1);
    expect(wrapper.emitted("toggleFavorite")).toHaveLength(1);
    expect(wrapper.emitted("showDescription")).toHaveLength(1);
  });

  it("renders info section details and emits edit", async () => {
    const wrapper = mount(GameDetailInfoSection, {
      props: {
        game: createTestGame({
          description: "A secret government building.",
          released: "2019-08-27",
          platforms: "PC",
          developers: "Remedy",
          publishers: "505 Games",
          exe_path: "C:\\Games\\Control\\Control.exe",
        }),
      },
    });

    expect(wrapper.text()).toContain("Remedy");
    expect(wrapper.text()).toContain("505 Games");
    expect(wrapper.text()).toContain("Control.exe");

    await wrapper.get("button").trigger("click");
    expect(wrapper.emitted("edit")).toHaveLength(1);
  });

  it("renders state section and emits save/update contracts", async () => {
    const wrapper = mount(GameDetailStateSection, {
      props: {
        playStatus: "not_started" as const,
        userRating: 4,
        userNote: "Remember the maze.",
        totalPlaytime: 7200,
        savingRating: false,
        savingNote: false,
        "onUpdate:playStatus": (
          value: "not_started" | "in_progress" | "completed" | "abandoned",
        ) =>
          wrapper.setProps({ playStatus: value }),
        "onUpdate:userRating": (value: number | null) =>
          wrapper.setProps({ userRating: value }),
        "onUpdate:userNote": (value: string) =>
          wrapper.setProps({ userNote: value }),
      },
    });

    expect(wrapper.text()).toContain("2 ч");

    await wrapper.get("select").setValue("completed");
    expect(wrapper.emitted("update:playStatus")?.[0]).toEqual(["completed"]);
    expect(wrapper.emitted("savePlayStatus")).toHaveLength(1);

    await wrapper.get("input[type='number']").setValue("6");
    expect(wrapper.emitted("update:userRating")?.[0]).toEqual([6]);

    const buttons = wrapper.findAll("button");
    await buttons[0]?.trigger("click");
    await buttons[1]?.trigger("click");
    expect(wrapper.emitted("saveUserRating")).toHaveLength(1);
    expect(wrapper.emitted("openRatingModal")).toHaveLength(1);

    await wrapper.get("textarea").setValue("Updated note");
    expect(wrapper.emitted("update:userNote")?.[0]).toEqual(["Updated note"]);
    await buttons[2]?.trigger("click");
    expect(wrapper.emitted("saveUserNote")).toHaveLength(1);
  });

  it("renders backup state and emits backup/save-path actions", async () => {
    const wrapper = mount(GameDetailBackupSection, {
      props: {
        savePathDraft: "{PATHTOGAME}\\Saves",
        "onUpdate:savePathDraft": (value: string) =>
          wrapper.setProps({ savePathDraft: value }),
        showAllBackups: false,
        "onUpdate:showAllBackups": (value: boolean) =>
          wrapper.setProps({ showAllBackups: value }),
        backupEnabled: true,
        creatingBackup: false,
        savingPath: false,
        locatingSavePath: false,
        canOpenSavePath: true,
        savePathPreviewParts: ["D:\\Games\\Control", "\\Saves"],
        backups,
        latestBackup: backups[0] ?? null,
        olderBackups: backups.slice(1),
        loadingBackups: false,
        restoring: false,
      },
    });

    expect(wrapper.text()).toContain("2.0 КБ");

    await wrapper.get("input[type='text'], input:not([type])").setValue("D:/Saves");
    expect(wrapper.emitted("update:savePathDraft")?.[0]).toEqual(["D:/Saves"]);

    await wrapper.get("input[type='checkbox']").setValue(false);
    expect(wrapper.emitted("toggleBackupEnabled")?.[0]).toEqual([false]);

    const buttons = wrapper.findAll("button");
    await buttons[0]?.trigger("click");
    expect(wrapper.emitted("createBackup")).toHaveLength(1);

    await buttons[8]?.trigger("click");
    expect(wrapper.emitted("update:showAllBackups")?.[0]).toEqual([true]);

    await buttons[7]?.trigger("click");
    expect(wrapper.emitted("restoreBackup")?.[0]).toEqual(["backup-latest"]);
  });

  it("renders metadata summary and emits metadata actions", async () => {
    const wrapper = mount(GameDetailMetadataSection, {
      props: {
        game: createTestGame({
          rawg_id: 42,
          metacritic: 85,
          background_image: "https://cdn.example.com/background.jpg",
          cover_image: "https://cdn.example.com/cover.jpg",
        }),
      },
    });

    expect(wrapper.text()).toContain("RAWG ID");
    expect(wrapper.text()).toContain("42");
    expect(wrapper.text()).toContain("85");

    const buttons = wrapper.findAll("button");
    await buttons[0]?.trigger("click");
    await buttons[1]?.trigger("click");

    expect(wrapper.emitted("searchRawg")).toHaveLength(1);
    expect(wrapper.emitted("edit")).toHaveLength(1);
  });

  it("renders description and rating modals with close/save contracts", async () => {
    const description = mount(GameDescriptionModal, {
      props: {
        open: true,
        description: "A brutalist action game.",
      },
      ...withTeleportTarget(),
    });

    expect(document.body.textContent).toContain("A brutalist action game.");
    await clickBodyButton(0);
    expect(description.emitted("close")).toHaveLength(1);
    description.unmount();

    const rating = mount(GameRatingModal, {
      props: {
        open: true,
        ratingDraft: 9,
        "onUpdate:open": (value: boolean) => rating.setProps({ open: value }),
        "onUpdate:ratingDraft": (value: number) =>
          rating.setProps({ ratingDraft: value }),
      },
      ...withTeleportTarget(),
    });

    await clickBodyButton(2);
    expect(rating.emitted("save")?.[0]).toEqual([7]);
    expect(rating.emitted("update:open")?.[0]).toEqual([false]);
  });

  it("emits edit dialog save, close, image search, and model updates", async () => {
    const wrapper = mount(GameEditDialog, {
      props: {
        open: true,
        gameName: "Control",
        saving: false,
        name: "Control",
        description: "Old description",
        backgroundImage: "",
        coverImage: "",
        "onUpdate:name": (value: string) => wrapper.setProps({ name: value }),
        "onUpdate:description": (value: string) =>
          wrapper.setProps({ description: value }),
        "onUpdate:backgroundImage": (value: string) =>
          wrapper.setProps({ backgroundImage: value }),
        "onUpdate:coverImage": (value: string) =>
          wrapper.setProps({ coverImage: value }),
      },
      ...withTeleportTarget(),
    });

    await setBodyInputValue("#edit-name", "Control Ultimate");
    expect(wrapper.emitted("update:name")?.[0]).toEqual(["Control Ultimate"]);
    await wrapper.setProps({ name: "Control Ultimate" });

    await clickBodyButton(0);
    await clickBodyButton(1);
    await clickBodyButton(2);

    expect(wrapper.emitted("save")).toHaveLength(1);
    expect(wrapper.emitted("close")).toHaveLength(1);
    expect(wrapper.emitted("searchImage")?.[0]).toEqual([
      { query: "Control", target: "background" },
    ]);
  });

  it("renders metadata search results and emits query, rename, search, and apply", async () => {
    const wrapper = mount(GameMetadataSearchModal, {
      props: {
        open: true,
        query: "Control",
        rename: false,
        results: [rawgResult],
        searching: false,
        applying: false,
        "onUpdate:open": (value: boolean) => wrapper.setProps({ open: value }),
        "onUpdate:query": (value: string) => wrapper.setProps({ query: value }),
        "onUpdate:rename": (value: boolean) =>
          wrapper.setProps({ rename: value }),
      },
      ...withTeleportTarget(),
    });

    expect(document.body.textContent).toContain("Control");
    expect(document.body.textContent).toContain("85");

    await setBodyInputValue("input:not([type])", "Alan Wake");
    await setBodyCheckboxValue("input[type='checkbox']", true);
    expect(wrapper.emitted("update:query")?.[0]).toEqual(["Alan Wake"]);
    expect(wrapper.emitted("update:rename")?.[0]).toEqual([true]);

    await clickBodyButton(0);
    await clickBodyButton(1);

    expect(wrapper.emitted("search")).toHaveLength(1);
    expect(wrapper.emitted("apply")?.[0]).toEqual([rawgResult]);
  });

  it("renders backup progress stage labels and counts", () => {
    const wrapper = mount(BackupProgressToast, {
      props: {
        progress: {
          active: true,
          stage: "restore",
          message: "Restoring files",
          done: 3,
          total: 9,
        },
      },
    });

    expect(wrapper.text()).toContain("Восстановление бэкапа");
    expect(wrapper.text()).toContain("Restoring files");
    expect(wrapper.text()).toContain("3/9");
  });
});
