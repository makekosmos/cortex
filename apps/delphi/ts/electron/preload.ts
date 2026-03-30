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
  db: {
    loadAll: () => ipcRenderer.invoke('db:loadAll'),
    upsertTodo: (todo: unknown) => ipcRenderer.invoke('db:upsertTodo', todo),
    deleteTodo: (id: string) => ipcRenderer.invoke('db:deleteTodo', id),
    batchUpsertTodos: (todos: unknown[]) => ipcRenderer.invoke('db:batchUpsertTodos', todos),
    upsertProject: (project: unknown) => ipcRenderer.invoke('db:upsertProject', project),
    deleteProject: (id: string) => ipcRenderer.invoke('db:deleteProject', id),
    getSyncKv: (key: string) => ipcRenderer.invoke('db:getSyncKv', key),
    setSyncKv: (key: string, value: string) => ipcRenderer.invoke('db:setSyncKv', key, value),
    clearAll: () => ipcRenderer.invoke('db:clearAll'),
  },
  invoke: (channel: string, ...args: unknown[]) =>
    ipcRenderer.invoke(channel, ...args),
  on: (channel: string, listener: (...args: unknown[]) => void) => {
    const handler = (_event: Electron.IpcRendererEvent, ...args: unknown[]) =>
      listener(...args);
    ipcRenderer.on(channel, handler);
    return () => {
      ipcRenderer.removeListener(channel, handler);
    };
  },
});
