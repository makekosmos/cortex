import {
  CURRENT_STAMP_VERSION,
  shouldRebuildNative,
} from "@/lib/native-rebuild";

describe("scripts/rebuild-native-if-needed", () => {
  const baseState = {
    stampVersion: CURRENT_STAMP_VERSION,
    platform: "win32",
    arch: "x64",
    electronVersion: "41.2.0",
    betterSqlite3Version: "12.9.0",
    artifactPath: "D:/repo/node_modules/better-sqlite3/build/Release/better_sqlite3.node",
    artifactExists: true,
    artifactSize: 1918464,
    artifactMtimeMs: 123456,
  };

  const matchingStamp = {
    stampVersion: baseState.stampVersion,
    platform: baseState.platform,
    arch: baseState.arch,
    electronVersion: baseState.electronVersion,
    betterSqlite3Version: baseState.betterSqlite3Version,
    artifactPath: baseState.artifactPath,
    artifactSize: baseState.artifactSize,
    artifactMtimeMs: baseState.artifactMtimeMs,
  };

  it("rebuilds when no stamp is present", () => {
    const decision = shouldRebuildNative(null, baseState);

    expect(decision.shouldRebuild).toBe(true);
    expect(decision.reasons).toContain("rebuild stamp is missing");
  });

  it("skips rebuild when the artifact and stamp still match", () => {
    const decision = shouldRebuildNative(matchingStamp, baseState);

    expect(decision.shouldRebuild).toBe(false);
    expect(decision.reasons).toHaveLength(0);
  });

  it("rebuilds when the native artifact is missing", () => {
    const decision = shouldRebuildNative(matchingStamp, {
      ...baseState,
      artifactExists: false,
      artifactSize: null,
      artifactMtimeMs: null,
    });

    expect(decision.shouldRebuild).toBe(true);
    expect(decision.reasons).toContain("native artifact is missing");
  });

  it.each([
    [
      "stamp version changed",
      { stampVersion: CURRENT_STAMP_VERSION - 1 },
    ],
    [
      "platform changed",
      { platform: "linux" },
    ],
    [
      "architecture changed",
      { arch: "arm64" },
    ],
    [
      "Electron version changed",
      { electronVersion: "40.0.0" },
    ],
    [
      "better-sqlite3 version changed",
      { betterSqlite3Version: "12.8.0" },
    ],
    [
      "artifact path changed",
      { artifactPath: "D:/repo/other.node" },
    ],
    [
      "artifact size changed",
      { artifactSize: 1918000 },
    ],
    [
      "artifact mtime changed",
      { artifactMtimeMs: 654321 },
    ],
  ])("rebuilds when %s", (reason, stampOverride) => {
    const decision = shouldRebuildNative(
      {
        ...matchingStamp,
        ...stampOverride,
      },
      baseState,
    );

    expect(decision.shouldRebuild).toBe(true);
    expect(decision.reasons).toContain(reason);
  });
});
