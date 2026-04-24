import { opendir } from "node:fs/promises";
import path from "node:path";
import {
  isArrancadorSidecarUnavailableError,
  scanExecutablesWithSidecar,
} from "../sidecar/arrancador-sidecar";
import type { ExeEntry, ProcessEntry, ScanStreamOptions } from "./contracts";
import { listRunningProcesses } from "./helpers/process";
import {
  createScanAbortError,
  isExecutableFile,
  isHiddenSegment,
  isReadableDirectory,
  normalizeScanRoot,
} from "./helpers/scan";

export async function* scanExecutables(
  root: string,
  options: ScanStreamOptions = {},
): AsyncGenerator<ExeEntry> {
  const scanRoot = normalizeScanRoot(root);
  const stack = [scanRoot];

  while (stack.length > 0) {
    if (options.signal?.aborted) {
      throw createScanAbortError();
    }

    const currentDir = stack.pop();
    if (!currentDir || !(await isReadableDirectory(currentDir))) {
      continue;
    }

    const dir = await opendir(currentDir);
    try {
      for await (const entry of dir) {
        if (options.signal?.aborted) {
          throw createScanAbortError();
        }

        if (isHiddenSegment(entry.name)) {
          continue;
        }

        const fullPath = path.join(currentDir, entry.name);
        if (entry.isDirectory()) {
          stack.push(fullPath);
          continue;
        }

        if (!entry.isFile() || !isExecutableFile(entry.name)) {
          continue;
        }

        yield {
          path: fullPath,
          file_name: entry.name,
        };
      }
    } finally {
      await Promise.resolve(dir.close()).catch(() => undefined);
    }
  }
}

export async function scanExecutablesStream(
  root: string,
  options: ScanStreamOptions = {},
): Promise<number> {
  if (process.env.ARRANCADOR_SCAN_BACKEND !== "ts") {
    try {
      return await scanExecutablesWithSidecar(root, options);
    } catch (error) {
      if (!isArrancadorSidecarUnavailableError(error)) {
        throw error;
      }
      console.warn(
        "[Arrancador] Rust scan sidecar unavailable, falling back to TypeScript scanner.",
      );
    }
  }

  let count = 0;

  for await (const entry of scanExecutables(root, options)) {
    count += 1;
    await options.onEntry?.(entry);
  }

  return count;
}

export async function getRunningProcesses(): Promise<ProcessEntry[]> {
  return listRunningProcesses();
}

export function createScanCancellation() {
  const controller = new AbortController();

  return {
    signal: controller.signal,
    cancel: () => controller.abort(),
  };
}
