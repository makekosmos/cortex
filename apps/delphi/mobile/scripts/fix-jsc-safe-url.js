#!/usr/bin/env node
/**
 * Patch jsc-safe-url to not crash on URLs with empty path (e.g. /?platform=android).
 * This is a known issue with Metro 0.83 + bun workspaces.
 */
const fs = require("fs");
const path = require("path");

// Find the file in bun's hoisted location
const candidates = [
  path.resolve(__dirname, "../node_modules/jsc-safe-url/index.js"),
  path.resolve(__dirname, "../../../../node_modules/.bun/jsc-safe-url@0.2.4/node_modules/jsc-safe-url/index.js"),
];

for (const filePath of candidates) {
  if (!fs.existsSync(filePath)) continue;

  let content = fs.readFileSync(filePath, "utf8");

  if (content.includes("has an empty path")) {
    content = content.replace(
      /throw new Error\(\s*`The given URL[^`]*has an empty path[^`]*`\s*\);/,
      "return urlToConvert;"
    );
    fs.writeFileSync(filePath, content);
    console.log("[fix-jsc-safe-url] Patched:", filePath);
  } else {
    console.log("[fix-jsc-safe-url] Already patched or not needed:", filePath);
  }
}
