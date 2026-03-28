interface ElectronFsAPI {
  exists(filePath: string): Promise<boolean>;
  readTextFile(filePath: string): Promise<string>;
  writeTextFile(filePath: string, contents: string): Promise<void>;
  mkdir(dirPath: string): Promise<void>;
}

interface ElectronAPI {
  fs: ElectronFsAPI;
  invoke(channel: string, ...args: unknown[]): Promise<unknown>;
  on(channel: string, listener: (...args: unknown[]) => void): () => void;
}

interface Window {
  electronAPI?: ElectronAPI;
}
