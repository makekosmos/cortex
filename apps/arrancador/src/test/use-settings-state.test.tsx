import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useSettingsState } from "@/hooks/useSettingsState";
import { arrancadorInvokeMock } from "./bridge";
import type { AppSettings } from "@/types";

const { settingsApiMock, metadataApiMock, backupApiMock } = vi.hoisted(() => ({
  settingsApiMock: { getAll: vi.fn(), update: vi.fn() },
  metadataApiMock: { setApiKey: vi.fn() },
  backupApiMock: { refreshSqobaManifest: vi.fn() },
}));

vi.mock("@/lib/api", () => ({
  settingsApi: settingsApiMock,
  metadataApi: metadataApiMock,
  backupApi: backupApiMock,
}));

const baseSettings: AppSettings = {
  theme: "dark",
  ludusavi_path: "native",
  backup_directory: "C:\\Backups",
  auto_backup: true,
  backup_before_launch: true,
  backup_compression_enabled: true,
  backup_compression_level: 60,
  backup_skip_compression_once: true,
  max_backups_per_game: 5,
  rawg_api_key: "rawg-key",
  start_minimized_in_tray: false,
};

function Harness() {
  const s = useSettingsState();
  return (
    <div>
      <div data-testid="loading">{s.loading ? "yes" : "no"}</div>
      <div data-testid="saving">{s.saving ? "yes" : "no"}</div>
      <div data-testid="backupDir">{s.backupDirectory}</div>
      <div data-testid="compression">{s.compressionEnabled ? "on" : "off"}</div>
      <div data-testid="skipOnce">{s.skipCompressionOnce ? "yes" : "no"}</div>
      <div data-testid="level">{String(s.compressionLevel)}</div>
      <div data-testid="max">{String(s.maxBackups)}</div>
      <div data-testid="autoStart">{s.autoStart ? "on" : "off"}</div>

      <button type="button" onClick={() => s.handleCompressionToggle(false)}>
        compressOff
      </button>
      <button type="button" onClick={() => s.handleCompressionToggle(true)}>
        compressOn
      </button>
      <button type="button" onClick={() => s.handleCompressionLevelChange(999)}>
        levelHigh
      </button>
      <button
        type="button"
        onClick={() => s.handleCompressionLevelChange(Number.NaN)}
      >
        levelNaN
      </button>
      <button type="button" onClick={() => s.handleMaxBackupsChange(0)}>
        maxLow
      </button>
      <button
        type="button"
        onClick={() => s.handleMaxBackupsChange(Number.NaN)}
      >
        maxNaN
      </button>
      <button type="button" onClick={() => s.toggleAutoStart(true)}>
        autoStartOn
      </button>
      <button type="button" onClick={() => s.toggleAutoStart(false)}>
        autoStartOff
      </button>
      <button type="button" onClick={() => s.toggleAutoStart()}>
        autoStartToggle
      </button>
      <button type="button" onClick={() => s.selectBackupDirectory()}>
        pickDir
      </button>
      <button type="button" onClick={() => s.saveSettings()}>
        save
      </button>
      <button type="button" onClick={() => s.refreshSqobaManifest()}>
        refresh
      </button>
      <button type="button" onClick={() => s.setRawgApiKey("new")}>
        setRawg
      </button>
    </div>
  );
}

describe("useSettingsState", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    settingsApiMock.getAll.mockResolvedValue(baseSettings);
    settingsApiMock.update.mockResolvedValue(undefined);
    metadataApiMock.setApiKey.mockResolvedValue(undefined);
    backupApiMock.refreshSqobaManifest.mockResolvedValue(undefined);
    arrancadorInvokeMock.mockImplementation(async (...args) => {
      const [channel, payload] = args as [string, unknown?];
      if (channel === "get_autostart_state") {
        return false;
      }

      if (channel === "set_autostart_state") {
        return undefined;
      }

      if (channel === "dialog_open") {
        return null;
      }

      throw new Error(`Unexpected channel: ${String(channel)} ${String(payload)}`);
    });
  });

  it("loads settings and initializes derived state", async () => {
    render(<Harness />);
    expect(screen.getByTestId("loading")).toHaveTextContent("yes");

    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );
    expect(screen.getByTestId("backupDir")).toHaveTextContent("C:\\Backups");
    expect(screen.getByTestId("skipOnce")).toHaveTextContent("yes");
    expect(screen.getByTestId("autoStart")).toHaveTextContent("off");
    expect(arrancadorInvokeMock).toHaveBeenCalledWith("get_autostart_state");
  });

  it("handles compression toggle (disables skip once)", async () => {
    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );

    await userEvent.click(screen.getByRole("button", { name: "compressOff" }));
    expect(screen.getByTestId("compression")).toHaveTextContent("off");
    expect(screen.getByTestId("skipOnce")).toHaveTextContent("no");
  });

  it("does not clear skip-once when compression stays enabled", async () => {
    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );

    await userEvent.click(screen.getByRole("button", { name: "compressOn" }));
    expect(screen.getByTestId("compression")).toHaveTextContent("on");
    expect(screen.getByTestId("skipOnce")).toHaveTextContent("yes");
  });

  it("clamps numeric settings and ignores NaN", async () => {
    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );

    await userEvent.click(screen.getByRole("button", { name: "levelHigh" }));
    expect(screen.getByTestId("level")).toHaveTextContent("100");

    await userEvent.click(screen.getByRole("button", { name: "levelNaN" }));
    expect(screen.getByTestId("level")).toHaveTextContent("100");

    await userEvent.click(screen.getByRole("button", { name: "maxLow" }));
    expect(screen.getByTestId("max")).toHaveTextContent("1");

    await userEvent.click(screen.getByRole("button", { name: "maxNaN" }));
    expect(screen.getByTestId("max")).toHaveTextContent("1");
  });

  it("toggles autostart and rolls back on failures", async () => {
    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );

    await userEvent.click(screen.getByRole("button", { name: "autoStartOn" }));
    await waitFor(() =>
      expect(arrancadorInvokeMock).toHaveBeenCalledWith("set_autostart_state", {
        enabled: true,
      }),
    );
    expect(screen.getByTestId("autoStart")).toHaveTextContent("on");

    arrancadorInvokeMock.mockImplementationOnce(async () => {
      throw new Error("nope");
    });

    await userEvent.click(screen.getByRole("button", { name: "autoStartOff" }));
    await waitFor(() =>
      expect(arrancadorInvokeMock).toHaveBeenCalledWith("set_autostart_state", {
        enabled: false,
      }),
    );
    expect(screen.getByTestId("autoStart")).toHaveTextContent("on");
  });

  it("toggles autostart without an explicit next value", async () => {
    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );

    await userEvent.click(
      screen.getByRole("button", { name: "autoStartToggle" }),
    );
    await waitFor(() =>
      expect(arrancadorInvokeMock).toHaveBeenCalledWith("set_autostart_state", {
        enabled: true,
      }),
    );
    expect(screen.getByTestId("autoStart")).toHaveTextContent("on");
  });

  it("selects backup directory when dialog returns a path", async () => {
    arrancadorInvokeMock.mockImplementation(async (...args) => {
      const [channel] = args as [string];
      if (channel === "get_autostart_state") {
        return false;
      }

      if (channel === "dialog_open") {
        return "D:\\Backups";
      }

      return undefined;
    });

    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );

    await userEvent.click(screen.getByRole("button", { name: "pickDir" }));
    await waitFor(() =>
      expect(screen.getByTestId("backupDir")).toHaveTextContent("D:\\Backups"),
    );
  });

  it("does nothing when backup directory picker is canceled", async () => {
    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );

    await userEvent.click(screen.getByRole("button", { name: "pickDir" }));
    expect(screen.getByTestId("backupDir")).toHaveTextContent("C:\\Backups");
  });

  it("saves settings, calls metadataApi when RAWG key changed, then reloads", async () => {
    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );

    await userEvent.click(screen.getByRole("button", { name: "setRawg" }));
    await userEvent.click(screen.getByRole("button", { name: "save" }));

    await waitFor(() => expect(settingsApiMock.update).toHaveBeenCalled());
    expect(metadataApiMock.setApiKey).toHaveBeenCalledWith("new");
    await waitFor(() =>
      expect(settingsApiMock.getAll).toHaveBeenCalledTimes(2),
    );
  });

  it("saves settings without calling metadataApi when RAWG key is unchanged", async () => {
    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );

    await userEvent.click(screen.getByRole("button", { name: "save" }));

    await waitFor(() => expect(settingsApiMock.update).toHaveBeenCalled());
    expect(metadataApiMock.setApiKey).not.toHaveBeenCalled();
  });

  it("refreshes SQOBA manifest", async () => {
    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );
    await userEvent.click(screen.getByRole("button", { name: "refresh" }));
    expect(backupApiMock.refreshSqobaManifest).toHaveBeenCalled();
  });

  it("handles load/save errors without throwing", async () => {
    settingsApiMock.getAll.mockRejectedValueOnce(new Error("load-fail"));
    settingsApiMock.update.mockRejectedValueOnce(new Error("save-fail"));

    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );

    await userEvent.click(screen.getByRole("button", { name: "save" }));
    await waitFor(() =>
      expect(screen.getByTestId("saving")).toHaveTextContent("no"),
    );

    expect(errorSpy).toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it("logs and recovers when saveSettings fails after settings are loaded", async () => {
    settingsApiMock.update.mockRejectedValueOnce(new Error("save-fail"));
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(<Harness />);
    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("no"),
    );

    await userEvent.click(screen.getByRole("button", { name: "save" }));
    await waitFor(() =>
      expect(screen.getByTestId("saving")).toHaveTextContent("no"),
    );

    expect(settingsApiMock.update).toHaveBeenCalled();
    expect(errorSpy).toHaveBeenCalledWith(
      "Failed to save settings:",
      expect.any(Error),
    );
    errorSpy.mockRestore();
  });
});
