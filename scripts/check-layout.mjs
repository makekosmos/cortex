import { access } from "node:fs/promises";

const required = ["desktop/package.json", "runtime/Cargo.toml"];

for (const file of required) await access(file);
console.log(`shell layout ok (${required.length} ownership manifests)`);
