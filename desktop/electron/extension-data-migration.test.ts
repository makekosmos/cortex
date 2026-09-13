import { expect, test } from "../test-support/node-test.mjs";
import { mkdtemp, mkdir, readFile, rm, symlink, utimes, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { mergeLegacyExtensionData } from "./extension-data-migration";

async function tempRoots(): Promise<{
  root: string;
  canonical: string;
  arcadia: string;
  arrancador: string;
}> {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-extension-data-"));
  const canonical = path.join(root, "canonical");
  const arcadia = path.join(root, "arcadia");
  const arrancador = path.join(root, "arrancador");
  await Promise.all([mkdir(canonical), mkdir(arcadia), mkdir(arrancador)]);
  return { root, canonical, arcadia, arrancador };
}

test("merges Arcadia aliases in fixed order and preserves destination values", async () => {
  const roots = await tempRoots();
  try {
    await mkdir(path.join(roots.canonical, "nested"), { recursive: true });
    await mkdir(path.join(roots.arcadia, "nested"), { recursive: true });
    await mkdir(path.join(roots.arrancador, "nested"), { recursive: true });
    await writeFile(
      path.join(roots.canonical, "settings.json"),
      JSON.stringify({
        keep: "canonical",
        nested: { keep: "canonical", deleted: null },
        array: ["canonical"],
      }),
    );
    await writeFile(
      path.join(roots.arcadia, "settings.json"),
      JSON.stringify({
        keep: "arcadia",
        nested: { keep: "arcadia", fromArcadia: true },
        array: ["arcadia"],
      }),
    );
    await writeFile(
      path.join(roots.arrancador, "settings.json"),
      JSON.stringify({
        keep: "arrancador",
        nested: { fromArrancador: true },
        array: ["arrancador"],
      }),
    );
    await writeFile(path.join(roots.arrancador, "new.bin"), Buffer.from([1, 2, 3]));

    const result = await mergeLegacyExtensionData({
      canonicalId: "com.kosmos.arcadia",
      destinationRoot: roots.canonical,
      sources: [
        { id: "arrancador", root: roots.arrancador },
        { id: "arcadia", root: roots.arcadia },
      ],
    });

    expect(result.copiedFiles).toBe(1);
    expect(result.mergedJsonFiles).toBe(2);
    expect(JSON.parse(await readFile(path.join(roots.canonical, "settings.json"), "utf8"))).toEqual(
      {
        keep: "canonical",
        nested: { keep: "canonical", deleted: null, fromArcadia: true, fromArrancador: true },
        array: ["canonical"],
      },
    );
    expect(await readFile(path.join(roots.canonical, "new.bin"))).toEqual(Buffer.from([1, 2, 3]));
  } finally {
    await rm(roots.root, { recursive: true, force: true });
  }
});

test("copies missing files with source mtime and keeps invalid, scalar, array, and conflicting files", async () => {
  const roots = await tempRoots();
  try {
    const mtime = new Date("2020-01-02T03:04:05.000Z");
    await writeFile(path.join(roots.arcadia, "missing.txt"), "source");
    await utimes(path.join(roots.arcadia, "missing.txt"), mtime, mtime);
    await writeFile(path.join(roots.canonical, "invalid.json"), "destination");
    await writeFile(path.join(roots.arcadia, "invalid.json"), "{broken");
    await writeFile(path.join(roots.canonical, "scalar.json"), "null");
    await writeFile(path.join(roots.arcadia, "scalar.json"), '{"new":true}');
    await writeFile(path.join(roots.canonical, "array.json"), '["destination"]');
    await writeFile(path.join(roots.arcadia, "array.json"), '["source"]');
    await writeFile(path.join(roots.canonical, "same.bin"), "destination");
    await writeFile(path.join(roots.arcadia, "same.bin"), "source");

    await mergeLegacyExtensionData({
      canonicalId: "com.kosmos.arcadia",
      destinationRoot: roots.canonical,
      sources: [{ id: "arcadia", root: roots.arcadia }],
    });

    expect(await readFile(path.join(roots.canonical, "missing.txt"), "utf8")).toBe("source");
    expect(
      (await import("node:fs/promises")).stat(path.join(roots.canonical, "missing.txt")),
    ).resolves.toMatchObject({ mtime });
    expect(await readFile(path.join(roots.canonical, "invalid.json"), "utf8")).toBe("destination");
    expect(await readFile(path.join(roots.canonical, "scalar.json"), "utf8")).toBe("null");
    expect(await readFile(path.join(roots.canonical, "array.json"), "utf8")).toBe(
      '["destination"]',
    );
    expect(await readFile(path.join(roots.canonical, "same.bin"), "utf8")).toBe("destination");
  } finally {
    await rm(roots.root, { recursive: true, force: true });
  }
});

test("rejects allowlist violations, overlapping roots, and symlink escapes before copying", async () => {
  const roots = await tempRoots();
  try {
    await writeFile(path.join(roots.arcadia, "safe.txt"), "safe");
    await expect(
      mergeLegacyExtensionData({
        canonicalId: "com.kosmos.arcadia",
        destinationRoot: roots.canonical,
        sources: [{ id: "__proto__", root: roots.arcadia }],
      }),
    ).rejects.toThrow("not allowlisted");

    await expect(
      mergeLegacyExtensionData({
        canonicalId: "com.kosmos.arcadia",
        destinationRoot: roots.canonical,
        sources: [{ id: "arcadia", root: path.join(roots.canonical, "nested") }],
      }),
    ).rejects.toThrow("overlap");

    if (process.platform !== "win32") {
      const outside = path.join(roots.root, "outside.txt");
      await writeFile(outside, "outside");
      await symlink(outside, path.join(roots.arcadia, "escape.txt"));
      await expect(
        mergeLegacyExtensionData({
          canonicalId: "com.kosmos.arcadia",
          destinationRoot: roots.canonical,
          sources: [{ id: "arcadia", root: roots.arcadia }],
        }),
      ).rejects.toThrow("symlink or junction");
    }
    await expect(readFile(path.join(roots.canonical, "safe.txt"))).rejects.toMatchObject({
      code: "ENOENT",
    });
  } finally {
    await rm(roots.root, { recursive: true, force: true });
  }
});

test("fills a deleted destination key while retaining nested null and arrays", async () => {
  const roots = await tempRoots();
  try {
    await writeFile(
      path.join(roots.canonical, "state.json"),
      JSON.stringify({ retained: null, list: [] }),
    );
    await writeFile(
      path.join(roots.arcadia, "state.json"),
      JSON.stringify({ deletedByUser: "restore-on-first-stage", retained: "source", list: [1, 2] }),
    );
    await mergeLegacyExtensionData({
      canonicalId: "com.kosmos.arcadia",
      destinationRoot: roots.canonical,
      sources: [{ id: "arcadia", root: roots.arcadia }],
    });
    expect(JSON.parse(await readFile(path.join(roots.canonical, "state.json"), "utf8"))).toEqual({
      retained: null,
      list: [],
      deletedByUser: "restore-on-first-stage",
    });
  } finally {
    await rm(roots.root, { recursive: true, force: true });
  }
});
