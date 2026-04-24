import path from "node:path";
import { describe, expect, it } from "vitest";

import {
  ArrancadorSidecarUnavailableError,
  getArrancadorSidecarBinaryPath,
  scanExecutablesWithSidecar,
} from "./arrancador-sidecar";

const binaryName = process.platform === "win32"
  ? "arrancador-sidecar.exe"
  : "arrancador-sidecar";

describe("getArrancadorSidecarBinaryPath", () => {
  it("resolves packaged sidecar from resourcesPath", () => {
    expect(
      getArrancadorSidecarBinaryPath({
        isPackaged: true,
        resourcesPath: "C:\\App\\resources",
      }),
    ).toBe(path.join("C:\\App\\resources", "arrancador-sidecar", binaryName));
  });

  it("resolves dev sidecar from app root target dir", () => {
    expect(
      getArrancadorSidecarBinaryPath({
        appRoot: "C:\\Repo\\apps\\arrancador",
        isPackaged: false,
      }),
    ).toBe(
      path.join(
        "C:\\Repo\\apps\\arrancador",
        "sidecar",
        "target",
        "debug",
        binaryName,
      ),
    );
  });
});

describe("scanExecutablesWithSidecar", () => {
  it("reports an unavailable sidecar for a missing binary", async () => {
    await expect(
      scanExecutablesWithSidecar("C:\\missing", {
        binaryPath: path.join("C:\\definitely-missing", binaryName),
      }),
    ).rejects.toBeInstanceOf(ArrancadorSidecarUnavailableError);
  });
});
