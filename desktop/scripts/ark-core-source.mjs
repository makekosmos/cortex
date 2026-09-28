import path from "node:path";
import { fileURLToPath } from "node:url";

export const ARK_CORE_SOURCE = "core/crates/ark-core";
const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
export const ARK_CORE_SOURCE_DIR = path.join(repoRoot, ARK_CORE_SOURCE);
