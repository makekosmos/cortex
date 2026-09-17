// Test-only fixture keys. They are never accepted by a production build.
export const TEST_ONLY_ROOT = {
  privateKey:
    "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIJaqYBUS6pxYArjIJFIVFSBqYEckyyyneug7j00UAjye\n-----END PRIVATE KEY-----\n",
  publicKey: "kO9VLTsXJ61nEokkkuDWvlh7iar3IChCTX1NITSlCLU=",
};
export const TEST_ONLY_RELEASE = {
  privateKey:
    "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIOySB4fj+9fjjYqVGN0MUgvLCskThB42RZM33lFKNGId\n-----END PRIVATE KEY-----\n",
  publicKey: "38NFuh0ZiMf4CFadsii2MYZhb3+nIZgqMelepE8Xjho=",
};
export const TEST_ONLY_STORE = {
  privateKey:
    "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEILJip2sm44kmthTuVD7jEcexS2Pvbq8NXSRjTxr5Nop0\n-----END PRIVATE KEY-----\n",
  publicKey: "WfIKwKU5F/RyZZWPhZXNmuiDjIU8yTw1BiYC0XdQDSQ=",
  keyId: "store-test",
};
