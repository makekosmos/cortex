interface ElectronFsAPI {
  exists(filePath: string): Promise<boolean>;
  readTextFile(filePath: string): Promise<string>;
  writeTextFile(filePath: string, contents: string): Promise<void>;
  mkdir(dirPath: string): Promise<void>;
}

interface ElectronAPI {
  fs: ElectronFsAPI;
}

interface Window {
  electronAPI?: ElectronAPI;
}
