import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const packageJson = JSON.parse(readFileSync(path.join(scriptsDir, "..", "package.json"), "utf8"));

test("Desktop uses the Kosmos package and build identity", () => {
  expect(packageJson.name).toBe("kosmos-desktop");
  expect(packageJson.build.productName).toBe("CosCast");
  expect(packageJson.build.artifactName).not.toContain("kepler-shell");
});
