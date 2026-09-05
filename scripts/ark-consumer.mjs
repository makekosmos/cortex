import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const token = process.env.NODE_AUTH_TOKEN ?? process.env.GITHUB_TOKEN;
if (!token)
  throw new Error("published ARK consumer check requires NODE_AUTH_TOKEN or GITHUB_TOKEN");

const root = mkdtempSync(path.join(os.tmpdir(), "makekosmos-ark-consumer-"));
const userConfig = path.join(root, ".npmrc");
try {
  writeFileSync(
    path.join(root, "package.json"),
    JSON.stringify(
      {
        name: "@makekosmos/ark-consumer-fixture",
        private: true,
        type: "module",
        dependencies: { "@makekosmos/ark": "0.1.1" },
        devDependencies: { typescript: "6.0.3" },
      },
      null,
      2,
    ),
  );
  writeFileSync(
    path.join(root, "tsconfig.json"),
    JSON.stringify(
      {
        compilerOptions: {
          module: "NodeNext",
          moduleResolution: "NodeNext",
          target: "ES2022",
          strict: true,
          noEmit: true,
        },
        include: ["consumer.ts"],
      },
      null,
      2,
    ),
  );
  writeFileSync(
    path.join(root, "consumer.ts"),
    `import { ArkClient, createArkAgentsApi } from "@makekosmos/ark";
import { createArkAgentsApi as createAgentsSubpath } from "@makekosmos/ark/agents";
import type { ArkAgentsApi, ArkClientOptions, TodoItem } from "@makekosmos/ark";

const clientConstructor: typeof ArkClient = ArkClient;
const rootAgentsFactory: typeof createArkAgentsApi = createArkAgentsApi;
const subpathAgentsFactory: typeof createAgentsSubpath = createAgentsSubpath;
const options: ArkClientOptions = { spaceId: "fixture-space", deviceId: "fixture-device" };
const agentApi: ArkAgentsApi | undefined = undefined;
const item: TodoItem | undefined = undefined;
void [clientConstructor, rootAgentsFactory, subpathAgentsFactory, options, agentApi, item];
`,
  );
  writeFileSync(
    userConfig,
    "@makekosmos:registry=https://npm.pkg.github.com\n" +
      "//npm.pkg.github.com/:_authToken=" +
      token +
      "\n",
  );
  const env = { ...process.env, NPM_CONFIG_USERCONFIG: userConfig };
  execFileSync("npm", ["install", "--ignore-scripts", "--no-audit", "--no-fund"], {
    cwd: root,
    env,
    stdio: "inherit",
  });
  execFileSync(
    process.execPath,
    [path.join(root, "node_modules", "typescript", "bin", "tsc"), "--noEmit", "--pretty", "false"],
    { cwd: root, env, stdio: "inherit" },
  );
  execFileSync(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      `const root = await import("@makekosmos/ark");
const agents = await import("@makekosmos/ark/agents");
if (typeof root.ArkClient !== "function" || typeof root.createArkAgentsApi !== "function") throw new Error("published root exports are unavailable");
if (typeof agents.createArkAgentsApi !== "function") throw new Error("published agents export is unavailable");`,
    ],
    { cwd: root, env, stdio: "inherit" },
  );
  console.log("published @makekosmos/ark@0.1.1 consumer fixture passed");
} finally {
  rmSync(root, { recursive: true, force: true });
}
