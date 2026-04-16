import { app, screen } from "electron";
import type { SystemGpuInfo, SystemMonitorInfo } from "../contracts";

export async function collectElectronGpuInfo(): Promise<SystemGpuInfo[]> {
  if (!app.isReady()) {
    return [];
  }

  try {
    const info = await app.getGPUInfo("complete");
    const devices = Array.isArray((info as { gpuDevice?: unknown[] }).gpuDevice)
      ? ((info as { gpuDevice: Array<Record<string, unknown>> }).gpuDevice)
      : [];

    return devices.map((device, index) => ({
      name:
        (typeof device.deviceString === "string" && device.deviceString) ||
        (typeof device.vendorString === "string" && device.vendorString) ||
        `GPU ${index + 1}`,
      device_name:
        (typeof device.deviceString === "string" && device.deviceString) ||
        (typeof device.vendorString === "string" && device.vendorString) ||
        "",
      is_primary: Boolean(device.active ?? index === 0),
    }));
  } catch {
    return [];
  }
}

export function collectElectronMonitorInfo(): SystemMonitorInfo[] {
  try {
    const primaryId = screen.getPrimaryDisplay().id;
    return screen.getAllDisplays().map((display, index) => ({
      name: display.id === primaryId ? "Primary display" : `Display ${index + 1}`,
      device_name: String(display.id),
      width: display.size.width,
      height: display.size.height,
      refresh_rate: Math.round((display as { displayFrequency?: number }).displayFrequency ?? 0),
      is_primary: display.id === primaryId,
    }));
  } catch {
    return [];
  }
}
