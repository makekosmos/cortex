import { describe, expect, test } from "../../test-support/node-test.mjs";
import type { InstalledStoreItem, StoreListing } from "./manager-api";
import {
  installTarget,
  installKey,
  installedForListing,
  latestInstalledPackages,
  listingSupportsPlatform,
  packageAction,
} from "./store-helpers";

const listing = (id: string, packageId: string): StoreListing => ({
  id,
  kind: "kosmos-package",
  name: id,
  categories: ["notes"],
  availability: { platforms: ["windows"] },
  data_compatibility: [
    {
      type: "com.kosmos.note",
      versions: "^1",
      roles: ["read"],
      fidelity: "native",
    },
  ],
  distribution: { package_id: packageId, version: "1.0.0" },
});
const installed = (id: string): InstalledStoreItem => ({
  id,
  version: "1.0.0",
  kind: "app",
  publisher: "Kosmos",
  revoked: false,
  enabled: true,
  update_version: "1.1.0",
});

describe("store helpers", () => {
  test("matches listing by distribution package id and targets update", () => {
    const item = installed("pkg.notes");
    expect(installedForListing(listing("listing.notes", "pkg.notes"), [item])).toBe(item);
    expect(installTarget(listing("listing.notes", "pkg.notes"), item)).toEqual({
      package_id: "pkg.notes",
      version: "1.1.0",
    });
    expect(installKey(listing("listing.notes", "pkg.notes"), item)).toBe("pkg.notes");
  });

  test("uses the latest installed version for an app update", () => {
    const previous = { ...installed("pkg.notes"), version: "1.0.0" };
    const current = { ...installed("pkg.notes"), version: "1.1.0", update_version: null };

    expect(installedForListing(listing("listing.notes", "pkg.notes"), [previous, current])).toBe(
      current,
    );
    expect(latestInstalledPackages([previous, current])).toEqual([current]);
  });

  test("matches installed canonical apps when the catalog omits distribution", () => {
    const dictation = {
      ...listing("com.kosmos.dictation", "unused"),
      distribution: undefined,
    };
    const item = installed("com.kosmos.dictation");

    expect(installedForListing(dictation, [item])).toBe(item);
    expect(installTarget(dictation, item)).toEqual({
      package_id: "com.kosmos.dictation",
      version: "1.1.0",
    });
  });

  test("gates listings on the host platform token", () => {
    const windowsOnly = listing("listing.windows", "pkg.windows");
    const portable = {
      ...listing("listing.portable", "pkg.portable"),
      availability: { platforms: ["windows", "linux"] },
    };
    const undeclared = { ...listing("listing.none", "pkg.none"), availability: undefined };

    expect(listingSupportsPlatform(windowsOnly, "linux")).toBe(false);
    expect(listingSupportsPlatform(portable, "linux")).toBe(true);
    expect(listingSupportsPlatform(undeclared, "linux")).toBe(false);
    // No host token → no platform gate (fixture/degraded contexts stay visible).
    expect(listingSupportsPlatform(windowsOnly)).toBe(true);
    expect(listingSupportsPlatform(undeclared)).toBe(true);
  });

  test("only exposes an install action when a target version exists", () => {
    const unpublished = {
      ...listing("com.kosmos.graph", "unused"),
      distribution: undefined,
    };
    const current = { ...installed("com.kosmos.graph"), update_version: null };

    expect(installTarget(unpublished)).toBeNull();
    expect(packageAction(unpublished)).toBeNull();
    expect(packageAction(unpublished, current)).toBe("open");
    expect(packageAction(listing("com.kosmos.shell", "com.kosmos.shell"))).toBe("install");
  });
});
