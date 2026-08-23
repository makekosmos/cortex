import path from "node:path";

export function resolvePackagedHostExecutable(managerResourcesPath: string): string {
  return path.resolve(managerResourcesPath, "..", "..", "host", "Kosmos Package Host.exe");
}

export function resolvePackagedRuntimeExecutable(managerResourcesPath: string): string {
  return path.resolve(managerResourcesPath, "..", "..", "..", "Kosmos Runtime.exe");
}
