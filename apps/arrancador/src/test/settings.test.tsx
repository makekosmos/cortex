import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/theme-provider";
import Settings from "@/pages/Settings";
import { arrancadorInvokeMock } from "./bridge";
import type { AppSettings } from "@/types";

const { settingsApiMock, metadataApiMock } = vi.hoisted(() => ({
  settingsApiMock: {
    getAll: vi.fn(),
    update: vi.fn(),
  },
  metadataApiMock: {
    setApiKey: vi.fn(),
  },
}));

vi.mock("@/lib/api", () => ({
  settingsApi: settingsApiMock,
  metadataApi: metadataApiMock,
}));

const baseSettings: AppSettings = {
  theme: "dark",
  ludusavi_path: "native",
  backup_directory: "C:\\Backups",
  auto_backup: true,
  backup_before_launch: true,
  backup_compression_enabled: true,
  backup_compression_level: 60,
  backup_skip_compression_once: false,
  max_backups_per_game: 5,
  rawg_api_key: "rawg-key",
  start_minimized_in_tray: false,
};

const renderSettings = () =>
  render(
    <MemoryRouter>
      <ThemeProvider>
        <Settings />
      </ThemeProvider>
    </MemoryRouter>,
  );

describe("Settings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    settingsApiMock.getAll.mockResolvedValue(baseSettings);
    settingsApiMock.update.mockResolvedValue(undefined);
    metadataApiMock.setApiKey.mockResolvedValue(undefined);
    arrancadorInvokeMock.mockImplementation(async (...args) => {
      const [channel] = args as [string];
      if (channel === "get_autostart_state") {
        return false;
      }

      if (channel === "set_autostart_state") {
        return undefined;
      }

      if (channel === "dialog_open") {
        return null;
      }

      throw new Error(`Unexpected channel: ${String(channel)}`);
    });
  });

  it("loads settings and saves RAWG API changes", async () => {
    renderSettings();

    const apiInput = await screen.findByPlaceholderText("Ваш RAWG API ключ");
    expect(apiInput).toHaveValue("rawg-key");

    await userEvent.clear(apiInput);
    await userEvent.type(apiInput, "new-rawg-key");

    const saveButton = screen.getByRole("button", {
      name: "Сохранить настройки",
    });
    await userEvent.click(saveButton);

    await waitFor(() =>
      expect(settingsApiMock.update).toHaveBeenCalledWith(
        expect.objectContaining({ rawg_api_key: "new-rawg-key" }),
      ),
    );

    expect(metadataApiMock.setApiKey).toHaveBeenCalledWith("new-rawg-key");
  });

  it("toggles autostart through the electron bridge", async () => {
    renderSettings();

    const autostartSwitch = await screen.findByRole("switch", {
      name: "Автозапуск",
    });

    await userEvent.click(autostartSwitch);

    await waitFor(() =>
      expect(arrancadorInvokeMock).toHaveBeenCalledWith("set_autostart_state", {
        enabled: true,
      }),
    );
  });
});
