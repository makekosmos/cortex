import path from "node:path";
import { existsSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";

const arkSource = [
  path.resolve(process.cwd(), "../arca-sdk/src/index.ts"),
  path.resolve(process.cwd(), "../../arca-sdk/src/index.ts"),
  path.resolve(process.cwd(), "../../../arca-sdk/src/index.ts"),
].find(existsSync);
const aliases = new Map([
  ["@raycast/api", new URL("../packages/raycast-api/src/index.ts", import.meta.url).href],
  [
    "@raycast/api/jsx-runtime",
    new URL("../packages/raycast-api/src/jsx-runtime.ts", import.meta.url).href,
  ],
  ...(arkSource ? [["@kosmos/ark", pathToFileURL(arkSource).href]] : []),
]);

const extensions = [".ts", ".tsx", ".mjs", ".js"];

export async function resolve(specifier, context, nextResolve) {
  const alias = aliases.get(specifier);
  if (alias) return nextResolve(alias, context);
  try {
    return await nextResolve(specifier, context);
  } catch (error) {
    if (!specifier.startsWith(".")) throw error;
    if (path.extname(specifier) && path.extname(specifier) !== ".js") throw error;
    for (const candidate of [
      ...(path.extname(specifier) === ".js" ? [`${specifier.slice(0, -3)}.ts`] : []),
      ...extensions.map((extension) => `${specifier}${extension}`),
      ...extensions.map((extension) => `${specifier}/index${extension}`),
    ]) {
      try {
        return await nextResolve(candidate, context);
      } catch {
        // Try the next source extension.
      }
    }
    throw error;
  }
}

export async function load(url, context, nextLoad) {
  if (/[.](?:png|svg)$/.test(new URL(url).pathname)) {
    return {
      format: "module",
      shortCircuit: true,
      source: `export default ${JSON.stringify(fileURLToPath(url))};`,
    };
  }
  return nextLoad(url, context);
}
