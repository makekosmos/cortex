export const CURRENT_STAMP_VERSION = 1;

export type NativeState = {
  stampVersion: number;
  platform: string;
  arch: string;
  electronVersion: string;
  betterSqlite3Version: string;
  artifactPath: string;
  artifactExists: boolean;
  artifactSize: number | null;
  artifactMtimeMs: number | null;
};

export type NativeStamp = Omit<NativeState, "artifactExists"> & {
  artifactSize: number;
  artifactMtimeMs: number;
};

export type RebuildDecision = {
  shouldRebuild: boolean;
  reasons: string[];
};

export function shouldRebuildNative(
  savedStamp: NativeStamp | null,
  currentState: NativeState,
): RebuildDecision {
  const reasons: string[] = [];

  if (!currentState.artifactExists) {
    reasons.push("native artifact is missing");
  }

  if (!savedStamp) {
    reasons.push("rebuild stamp is missing");
  } else {
    if (savedStamp.stampVersion !== currentState.stampVersion) {
      reasons.push("stamp version changed");
    }
    if (savedStamp.platform !== currentState.platform) {
      reasons.push("platform changed");
    }
    if (savedStamp.arch !== currentState.arch) {
      reasons.push("architecture changed");
    }
    if (savedStamp.electronVersion !== currentState.electronVersion) {
      reasons.push("Electron version changed");
    }
    if (savedStamp.betterSqlite3Version !== currentState.betterSqlite3Version) {
      reasons.push("better-sqlite3 version changed");
    }
    if (savedStamp.artifactPath !== currentState.artifactPath) {
      reasons.push("artifact path changed");
    }
    if (savedStamp.artifactSize !== currentState.artifactSize) {
      reasons.push("artifact size changed");
    }
    if (savedStamp.artifactMtimeMs !== currentState.artifactMtimeMs) {
      reasons.push("artifact mtime changed");
    }
  }

  return {
    shouldRebuild: reasons.length > 0,
    reasons,
  };
}
