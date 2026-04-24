import { windowApi } from "@/lib/api";

export type WindowChromePlatform = "mac" | "windows" | "linux";

export function getFallbackWindowChromePlatform(): WindowChromePlatform {
  if (typeof navigator === "undefined") {
    return "windows";
  }

  if (navigator.platform.startsWith("Mac")) {
    return "mac";
  }

  if (navigator.platform.startsWith("Linux")) {
    return "linux";
  }

  return "windows";
}

function mapPlatform(platform: string): WindowChromePlatform {
  switch (platform) {
    case "darwin":
      return "mac";
    case "linux":
      return "linux";
    default:
      return "windows";
  }
}

export async function getWindowChromePlatform() {
  const platform = await windowApi.getPlatform();
  return mapPlatform(platform);
}

export async function minimizeWindow() {
  await windowApi.minimize();
}

export async function toggleMaximizeWindow() {
  await windowApi.toggleMaximize();
}

export async function closeWindow() {
  await windowApi.close();
}
