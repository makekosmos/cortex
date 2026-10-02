import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";

function scalar(value) {
  return value.trim().replace(/^(?:"([^"]*)"|'([^']*)')$/, "$1$2");
}

function sha512(file) {
  return createHash("sha512").update(readFileSync(file)).digest("base64");
}

// Windows is the only released platform, so the channel file is always
// latest.yml.
export function verifyLocalReleaseChannel(outputDir, expectedVersion) {
  const channelName = "latest.yml";
  const document = readFileSync(path.join(outputDir, channelName), "utf8");
  const version = /^version:\s*(.+?)\s*$/m.exec(document)?.[1];
  const primaryName = /^path:\s*(.+?)\s*$/m.exec(document)?.[1];
  const primaryHash = /^sha512:\s*(\S+)\s*$/m.exec(document)?.[1];
  if (!version || scalar(version) !== expectedVersion || !primaryName || !primaryHash)
    throw new Error(`${channelName} has invalid version or primary artifact metadata`);

  const files = [];
  let current;
  let inFiles = false;
  for (const line of document.split(/\r?\n/)) {
    if (/^files:\s*$/.test(line)) {
      inFiles = true;
      continue;
    }
    if (inFiles && /^\S/.test(line)) {
      if (current) files.push(current);
      current = undefined;
      inFiles = false;
    }
    if (!inFiles) continue;
    const url = /^\s*-\s+url:\s*(.+?)\s*$/.exec(line)?.[1];
    if (url) {
      if (current) files.push(current);
      current = { name: scalar(url) };
      continue;
    }
    const hash = /^\s+sha512:\s*(\S+)\s*$/.exec(line)?.[1];
    if (hash && current) current.sha512 = hash;
    const size = /^\s+size:\s*(\d+)\s*$/.exec(line)?.[1];
    if (size && current) current.size = Number(size);
  }
  if (current) files.push(current);
  if (files.length === 0) throw new Error(`${channelName} has no files`);

  const seen = new Set();
  for (const file of files) {
    if (
      !file.name ||
      path.basename(file.name) !== file.name ||
      seen.has(file.name) ||
      !file.sha512 ||
      !Number.isSafeInteger(file.size) ||
      file.size <= 0
    )
      throw new Error(`${channelName} has invalid file metadata`);
    seen.add(file.name);
    const artifact = path.join(outputDir, file.name);
    if (
      !existsSync(artifact) ||
      statSync(artifact).size !== file.size ||
      sha512(artifact) !== file.sha512
    )
      throw new Error(`${channelName} artifact mismatch: ${file.name}`);
  }

  const primary = scalar(primaryName);
  if (!seen.has(primary) || sha512(path.join(outputDir, primary)) !== primaryHash)
    throw new Error(`${channelName} primary artifact mismatch: ${primary}`);
}
