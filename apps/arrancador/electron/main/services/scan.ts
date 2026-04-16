import type { ExeEntry, ProcessEntry } from "./contracts";
import type { ScanStreamOptions } from "./contracts";
import {
  createScanAbortError,
  isExecutableFile,
  isHiddenSegment,
  isReadableDirectory,
  normalizeScanRoot,
} from "./helpers/scan";
import { listRunningProcesses } from "./helpers/process";
import { opendir } from "node:fs/promises";
import path from "node:path";

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
      await dir.close().catch(() => undefined);
    }
  }
}

export async function scanExecutablesStream(
  root: string,
  options: ScanStreamOptions = {},
): Promise<number> {
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
