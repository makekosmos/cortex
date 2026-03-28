// Polyfill for expo-router SSR references in SDK 55
if (typeof globalThis.document === "undefined") {
  // @ts-ignore — minimal stub to prevent ReferenceError
  globalThis.document = {};
}
