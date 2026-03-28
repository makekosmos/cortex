// Polyfill for expo-router SSR references in SDK 55
if (typeof globalThis.document === "undefined") {
  // @ts-expect-error — minimal stub to prevent ReferenceError
  globalThis.document = {
    createElement: () => ({}),
    addEventListener: () => {},
    removeEventListener: () => {},
    querySelector: () => null,
    querySelectorAll: () => [],
    head: { appendChild: () => {} },
    body: { appendChild: () => {} },
  };
}
