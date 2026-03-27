import { contextBridge, ipcRenderer } from 'electron';

contextBridge.exposeInMainWorld('electronAPI', {
  fs: {
    exists: (filePath: string) => ipcRenderer.invoke('fs:exists', filePath),
    readTextFile: (filePath: string) =>
      ipcRenderer.invoke('fs:readTextFile', filePath),
    writeTextFile: (filePath: string, contents: string) =>
      ipcRenderer.invoke('fs:writeTextFile', filePath, contents),
    mkdir: (dirPath: string) => ipcRenderer.invoke('fs:mkdir', dirPath),
  },
});
