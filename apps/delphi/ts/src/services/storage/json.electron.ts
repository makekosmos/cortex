import type { Task } from "@/types/task";

const DATA_DIR = "data";

const FILE_PATH = `${DATA_DIR}/tasks.json`;

function getElectronFs() {
  return window.electronAPI?.fs;
}

export async function ensureDataDir() {
  const fs = getElectronFs();

  if (!fs) return;

  const hasDir = await fs.exists(DATA_DIR);

  if (!hasDir) {
    await fs.mkdir(DATA_DIR);
  }
}

export async function loadFromJson(): Promise<Task[]> {
  const fs = getElectronFs();

  if (!fs) return [];

  const hasFile = await fs.exists(FILE_PATH);

  if (!hasFile) return [];

  const raw = await fs.readTextFile(FILE_PATH);

  const parsed = JSON.parse(raw);

  if (!Array.isArray(parsed?.tasks)) return [];

  return parsed.tasks.map((t: any) => ({
    ...t,

    created_at: new Date(t?.created_at ?? Date.now()),
  })) as Task[];
}

export async function saveToJson(tasks: Task[]) {
  const fs = getElectronFs();

  if (!fs) return;

  const payload = JSON.stringify({
    version: 1,

    lastUpdated: Date.now(),

    tasks,
  });

  await fs.writeTextFile(FILE_PATH, payload);
}
