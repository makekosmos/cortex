import { readFile } from "node:fs/promises";
import os from "node:os";

export async function getOsName() {
  if (process.platform === "linux") {
    try {
      const text = await readFile("/etc/os-release", "utf8");
      const pretty = /^(?:PRETTY_NAME|NAME)="([^"]+)"/m.exec(text)?.[1];
      if (pretty) {
        return pretty;
      }
    } catch {
      // ignored
    }
  }

  if (process.platform === "darwin") {
    return "macOS";
  }

  if (process.platform === "win32") {
    return "Windows";
  }

  return os.type();
}

export async function getOsVersion() {
  if (process.platform === "linux") {
    try {
      const text = await readFile("/etc/os-release", "utf8");
      const version = /^VERSION(?:_ID)?="([^"]+)"/m.exec(text)?.[1];
      if (version) {
        return version;
      }
    } catch {
      // ignored
    }
  }

  return os.release();
}

export function getCpuInfo() {
  const cpus = os.cpus();
  const first = cpus[0];
  const frequency = cpus.length
    ? Math.round(cpus.reduce((sum, cpu) => sum + cpu.speed, 0) / cpus.length)
    : 0;
  const logicalCoreFallback = cpus.length || 1;

  return {
    brand: first?.model ?? "Unknown",
    vendor_id: "",
    frequency_mhz: frequency,
    physical_cores: null as number | null,
    logical_cores: Math.max(
      1,
      os.availableParallelism?.() ?? logicalCoreFallback,
    ),
  };
}

export function getMemoryInfo() {
  return {
    total_bytes: os.totalmem(),
    used_bytes: os.totalmem() - os.freemem(),
    free_bytes: os.freemem(),
    available_bytes: os.freemem(),
    total_swap_bytes: 0,
    used_swap_bytes: 0,
  };
}

export function getSystemIdentity() {
  const uptimeSeconds = Math.floor(os.uptime());

  return {
    hostname: os.hostname() || null,
    os_name: null as string | null,
    os_version: null as string | null,
    kernel_version: os.release() || null,
    uptime_seconds: uptimeSeconds,
    boot_time: Math.max(0, Math.floor(Date.now() / 1000) - uptimeSeconds),
    arch: process.arch,
  };
}
