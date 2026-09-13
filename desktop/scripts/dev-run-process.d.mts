export interface ProcessInfo {
  pid: number;
  parentPid?: number;
  startTime: string;
  createdAtMs: number;
  commandLine: string;
}
export function processInfo(pid: number): ProcessInfo | null;
export function sameProcessStartTime(actual: string, recorded: string): boolean;
export function sameProcessIdentity(actual: ProcessInfo | null, expected: ProcessInfo): boolean;
export function processIdentityFromLock(
  lockPath: string,
  launchStartedAt: number,
  expectedExecutable: string,
  now?: number,
): ProcessInfo | null;
export function processTree(rootPid: number): Set<number>;
export function stopProcessTree(
  pid: number,
  expectedStartTime: string,
  expectedCommandLine: string,
  kind?: "run" | "backend",
): number[];
