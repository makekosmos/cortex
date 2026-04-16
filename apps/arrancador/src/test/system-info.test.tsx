import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import SystemInfoPage from "@/pages/SystemInfo";
import type { DiskSpeedResult, SystemInfo } from "@/types";

const { systemApiMock } = vi.hoisted(() => ({
  systemApiMock: {
    getInfo: vi.fn(),
    testDiskSpeed: vi.fn(),
  },
}));

vi.mock("@/lib/api", () => ({
  systemApi: systemApiMock,
}));

describe("SystemInfoPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  it("loads cached snapshot safely (including invalid JSON) and refreshes info + disk tests", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    window.localStorage.setItem("arrancador_system_info_snapshot", "{bad json");
    window.localStorage.setItem("arrancador_system_info_checked_at", "nope");

    const info1: SystemInfo = {
      hostname: null,
      os_name: "Windows",
      os_version: null,
      kernel_version: null,
      uptime_seconds: 0,
      boot_time: 0,
      arch: "x64",
      cpu: {
        brand: "CPU",
        vendor_id: "Genuine",
        frequency_mhz: 0,
        physical_cores: null,
        logical_cores: 0,
      },
      memory: {
        total_bytes: 0,
        used_bytes: 0,
        free_bytes: 0,
        available_bytes: 0,
        total_swap_bytes: 0,
        used_swap_bytes: 0,
      },
      disks: [
        {
          name: "DiskA",
          mount_point: "C:",
          file_system: "NTFS",
          total_bytes: 0,
          available_bytes: 0,
          kind: "SSD",
          is_removable: false,
          model: null,
          media_type: null,
        },
        {
          name: "DiskB",
          mount_point: "D:",
          file_system: "NTFS",
          total_bytes: 1024 * 1024 * 1024 * 11,
          available_bytes: 1024 * 1024 * 1024 * 5,
          kind: "HDD",
          is_removable: true,
          model: "ModelB",
          media_type: "USB",
        },
      ],
      gpus: [
        { name: "", device_name: "gpu0", is_primary: true },
        { name: "RTX", device_name: "gpu1", is_primary: false },
      ],
      monitors: [
        {
          name: "",
          device_name: "mon0",
          width: 0,
          height: 0,
          refresh_rate: 0,
          is_primary: true,
        },
      ],
    };

    const info2: SystemInfo = {
      ...info1,
      uptime_seconds: 60 * 60 * 25 + 60 * 13, // 1d 1h 13m
      cpu: { ...info1.cpu, physical_cores: 8, logical_cores: 16 },
      memory: {
        ...info1.memory,
        total_bytes: 1024,
        used_bytes: 4096, // intentionally > total to hit clamp
      },
    };

    const speedOk: DiskSpeedResult = {
      mount_point: "C:",
      size_bytes: 1024,
      write_mbps: 123.4,
      read_mbps: 456.7,
      elapsed_write_ms: 1,
      elapsed_read_ms: 1,
    };

    systemApiMock.getInfo
      .mockResolvedValueOnce(info1)
      .mockResolvedValueOnce(info2);
    systemApiMock.testDiskSpeed
      .mockResolvedValueOnce(speedOk)
      .mockRejectedValueOnce(new Error("disk-fail"));

    render(<SystemInfoPage />);

    // Invalid cache is handled and removed.
    await waitFor(() => expect(warnSpy).toHaveBeenCalled());
    expect(window.localStorage.getItem("arrancador_system_info_snapshot")).toBe(
      null,
    );

    // checkedAt invalid -> em dash
    expect(screen.getByText(/Последняя проверка:/)).toHaveTextContent("—");

    const refresh = screen.getByRole("button", { name: "Проверить снова" });
    await userEvent.click(refresh);
    await waitFor(() => expect(systemApiMock.getInfo).toHaveBeenCalledTimes(1));

    // Disk list renders and removable label is shown.
    expect(screen.getAllByText("Найдено 2").length).toBeGreaterThan(0);
    expect(screen.getByText("Съемный носитель")).toBeInTheDocument();

    // Non-removable disk test success.
    await userEvent.click(screen.getAllByText("Тест скорости")[0]);
    expect(await screen.findByText("123 MB/s")).toBeInTheDocument();
    expect(screen.getByText("457 MB/s")).toBeInTheDocument();

    // Next refresh brings a different CPU core label + uptime formatting.
    await userEvent.click(refresh);
    await waitFor(() => expect(systemApiMock.getInfo).toHaveBeenCalledTimes(2));
    expect(screen.getByText("8 физ. / 16 лог.")).toBeInTheDocument();
    expect(screen.getByText("1 д 1 ч 13 мин")).toBeInTheDocument();

    // Disk test failure path.
    await userEvent.click(screen.getAllByText("Тест скорости")[0]);
    expect(
      await screen.findByText("Не удалось провести тест"),
    ).toBeInTheDocument();

    warnSpy.mockRestore();
    errorSpy.mockRestore();
  });

  it("shows logical-only core label and surfaces refresh errors", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const info: SystemInfo = {
      hostname: null,
      os_name: "Windows",
      os_version: null,
      kernel_version: null,
      uptime_seconds: 0,
      boot_time: 0,
      arch: "x64",
      cpu: {
        brand: "CPU",
        vendor_id: "Genuine",
        frequency_mhz: 0,
        physical_cores: null,
        logical_cores: 16,
      },
      memory: {
        total_bytes: 0,
        used_bytes: 0,
        free_bytes: 0,
        available_bytes: 0,
        total_swap_bytes: 0,
        used_swap_bytes: 0,
      },
      disks: [],
      gpus: [],
      monitors: [],
    };

    systemApiMock.getInfo.mockResolvedValueOnce(info);

    render(<SystemInfoPage />);

    const refresh = await screen.findByRole("button", {
      name: "Проверить снова",
    });
    await userEvent.click(refresh);
    await waitFor(() => expect(systemApiMock.getInfo).toHaveBeenCalledTimes(1));

    expect(screen.getByText(/16\s+лог\./)).toBeInTheDocument();

    systemApiMock.getInfo.mockRejectedValueOnce(new Error("nope"));
    await userEvent.click(refresh);

    expect(
      await screen.findByText("Не удалось загрузить данные о системе"),
    ).toBeInTheDocument();

    errorSpy.mockRestore();
  });

  it("renders em-dash fallbacks and non-primary monitor label", async () => {
    const info: SystemInfo = {
      hostname: null,
      os_name: null,
      os_version: null,
      kernel_version: null,
      uptime_seconds: 0,
      boot_time: 0,
      arch: "x64",
      cpu: {
        brand: "",
        vendor_id: "",
        frequency_mhz: 0,
        physical_cores: 4,
        logical_cores: 8,
      },
      memory: {
        total_bytes: 0,
        used_bytes: 0,
        free_bytes: 0,
        available_bytes: 0,
        total_swap_bytes: 0,
        used_swap_bytes: 0,
      },
      disks: [],
      gpus: [],
      monitors: [
        {
          name: "Display 1",
          device_name: "mon0",
          width: 1920,
          height: 1080,
          refresh_rate: 60,
          is_primary: false,
        },
      ],
    };

    systemApiMock.getInfo.mockResolvedValueOnce(info);

    render(<SystemInfoPage />);

    const refresh = await screen.findByRole("button", {
      name: /Проверить снова/,
    });
    await userEvent.click(refresh);
    await waitFor(() => expect(systemApiMock.getInfo).toHaveBeenCalledTimes(1));

    // CPU fallbacks (vendor_id/brand empty) render as an em dash.
    expect(screen.getByText("Модель").parentElement).toHaveTextContent(
      "—",
    );

    // OS name/version empty -> join("") => em dash fallback.
    expect(screen.getByText("ОС").parentElement).toHaveTextContent("—");

    // Monitor non-primary label.
    expect(screen.getByText("Дополнительный")).toBeInTheDocument();
  });
});
