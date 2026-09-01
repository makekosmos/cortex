import { buildEngine } from "./fixtures/host-runtime";
import { TEST_ONLY_RELEASE, TEST_ONLY_ROOT } from "./fixtures/signing-keys";

export default function globalSetup() {
  buildEngine(
    {
      root: JSON.stringify({ key_id: "root", public_key: TEST_ONLY_ROOT.publicKey }),
      releases: JSON.stringify([{ key_id: "release-1", public_key: TEST_ONLY_RELEASE.publicKey }]),
    },
    true,
  );
}
