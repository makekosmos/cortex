import { buildEngine } from "../../host/e2e/fixtures/host-runtime";
import {
  TEST_ONLY_RELEASE,
  TEST_ONLY_ROOT,
  TEST_ONLY_STORE,
} from "../../host/e2e/fixtures/signing-keys";

export default function globalSetup() {
  buildEngine(
    {
      root: JSON.stringify({ key_id: "root", public_key: TEST_ONLY_ROOT.publicKey }),
      releases: JSON.stringify([{ key_id: "release-1", public_key: TEST_ONLY_RELEASE.publicKey }]),
      storeKeyId: TEST_ONLY_STORE.keyId,
      storePublicKey: TEST_ONLY_STORE.publicKey,
    },
    true,
  );
}
