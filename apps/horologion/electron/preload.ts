import { contextBridge, ipcRenderer } from "electron";
import type {
  CreateTimeEntryInput,
  StartTimerInput,
  HorologionApi,
  UpdateTimeEntryInput,
} from "../shared/ipc-types";

const api: HorologionApi = {
  timeEntries: {
    list: () => ipcRenderer.invoke("horologion:time-entries:list"),
    listRunning: () => ipcRenderer.invoke("horologion:time-entries:list-running"),
    startTimer: (input: StartTimerInput) => ipcRenderer.invoke("horologion:time-entries:start", input),
    stopTimer: (id: string) => ipcRenderer.invoke("horologion:time-entries:stop", id),
    update: (input: UpdateTimeEntryInput) => ipcRenderer.invoke("horologion:time-entries:update", input),
    create: (input: CreateTimeEntryInput) => ipcRenderer.invoke("horologion:time-entries:create", input),
    delete: (id: string) => ipcRenderer.invoke("horologion:time-entries:delete", id),
  },
  tags: {
    list: () => ipcRenderer.invoke("horologion:tags:list"),
  },
  tasks: {
    list: () => ipcRenderer.invoke("horologion:tasks:list"),
  },
  ark: {
    status: () => ipcRenderer.invoke("horologion:ark:status"),
  },
  settings: {
    open: () => ipcRenderer.invoke("horologion:settings:open"),
  },
};

contextBridge.exposeInMainWorld("horologion", api);
