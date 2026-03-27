import { isTauriRuntime } from '@/services/runtime/platform';

describe('runtime platform helpers', () => {
  beforeEach(() => {
    delete (window as Window & { __TAURI__?: unknown }).__TAURI__;
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it('detects non-tauri runtime by default', () => {
    expect(isTauriRuntime()).toBe(false);
  });

  it('detects tauri runtime when marker exists', () => {
    (window as Window & { __TAURI__?: unknown }).__TAURI__ = {};
    expect(isTauriRuntime()).toBe(true);
  });
});
