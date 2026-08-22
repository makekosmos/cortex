import path from "node:path";

export function resolvePackagedHostExecutable(managerResourcesPath: string): string {
  return path.resolve(managerResourcesPath, "..", "..", "host", "Kosmos Package Host.exe");
}
