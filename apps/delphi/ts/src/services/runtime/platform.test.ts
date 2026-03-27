import { isElectronRuntime } from '@/services/runtime/platform';

describe('runtime platform helpers', () => {
  beforeEach(() => {
    delete (window as Window & { electronAPI?: unknown }).electronAPI;
  });

  it('detects non-electron runtime by default', () => {
    expect(isElectronRuntime()).toBe(false);
  });

  it('detects electron runtime when electronAPI exists', () => {
    (window as Window & { electronAPI?: unknown }).electronAPI = {} as ElectronAPI;
    expect(isElectronRuntime()).toBe(true);
  });
});
