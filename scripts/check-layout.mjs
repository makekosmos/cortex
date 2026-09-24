import { access } from "node:fs/promises";

const required = [
  "desktop/package.json",
  "runtime/Cargo.toml",
  "native-services/kepler-focus-svc/Cargo.toml",
  "native-services/kepler-focus-helper/Cargo.toml",
  "native-services/kepler-watcher/Cargo.toml",
];

for (const file of required) await access(file);
console.log(`shell layout ok (${required.length} ownership manifests)`);
