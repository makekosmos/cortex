import type { DiskSpeedResult, SystemInfo, SystemServiceOptions } from "./contracts";
import { collectDiskInfos, testDiskSpeed as probeDiskSpeed } from "./helpers/disk";
import { collectElectronGpuInfo, collectElectronMonitorInfo } from "./helpers/electron";
import {
  getCpuInfo,
  getMemoryInfo,
  getOsName,
  getOsVersion,
  getSystemIdentity,
} from "./helpers/system";

export async function getSystemInfo(
  options: SystemServiceOptions = {},
): Promise<SystemInfo> {
  const identity = getSystemIdentity();
  const [osName, osVersion, disks, gpus, monitors] = await Promise.all([
    getOsName(),
    getOsVersion(),
    options.getDiskInfo?.() ?? collectDiskInfos(),
    options.getGpuInfo?.() ?? collectElectronGpuInfo(),
    options.getMonitorInfo?.() ?? collectElectronMonitorInfo(),
  ]);

  return {
    ...identity,
    os_name: osName,
    os_version: osVersion,
    cpu: getCpuInfo(),
    memory: getMemoryInfo(),
    disks,
    gpus,
    monitors,
  };
}

export async function testDiskSpeed(
  mountPoint: string,
): Promise<DiskSpeedResult> {
  return probeDiskSpeed(mountPoint);
}

export function createSystemService(options: SystemServiceOptions = {}) {
  return {
    getSystemInfo: () => getSystemInfo(options),
    testDiskSpeed,
  };
}
