import { strict as assert } from "node:assert";

const readme = await Bun.file("packages/arksync-node/README.md").text();

for (const text of [
  "# @arksync/node",
  "Self-managed sidecar",
  "Injected sidecar",
  "`dbPath` is required",
  "sends `init` automatically",
  "Relay options are intentionally rejected today",
  "await ark.start()",
  "await ark.objects.upsert",
  "await ark.objectTypes.upsert",
  "await ark.links.upsert",
  "await ark.usage.trackedApps.upsert",
  "should call this SDK instead of opening Ark SQLite",
]) {
  assert.ok(readme.includes(text), `README should include: ${text}`);
}

assert.ok(readme.length > 2000, "README should be substantial enough to guide integration");
console.log("verify-readme PASS");
