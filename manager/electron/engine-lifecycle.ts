import { access } from "node:fs/promises";

type AccessFile = (path: string) => Promise<void>;

export async function waitForEngineStopped(
  lockPath: string,
  accessFile: AccessFile = access,
): Promise<void> {
  for (let attempt = 0; attempt < 150; attempt += 1) {
    try {
      await accessFile(lockPath);
    } catch (error) {
      // SAFETY: Node filesystem errors expose the standard errno code here.
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
      throw error;
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error("Engine did not stop before ARK restore");
}
