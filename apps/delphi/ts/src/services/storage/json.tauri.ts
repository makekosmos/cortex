import {
  BaseDirectory,
  exists,
  mkdir,
  readTextFile,
  writeTextFile,
} from '@tauri-apps/plugin-fs';
import type { Task } from '@/types/task';

const DATA_DIR = 'data';
const FILE_PATH = `${DATA_DIR}/tasks.json`;

export async function ensureDataDir() {
  const hasDir = await exists(DATA_DIR, { baseDir: BaseDirectory.AppData });

  if (!hasDir) {
    await mkdir(DATA_DIR, { baseDir: BaseDirectory.AppData, recursive: true });
  }
}

export async function loadFromJson(): Promise<Task[]> {
  const hasFile = await exists(FILE_PATH, { baseDir: BaseDirectory.AppData });
  if (!hasFile) return [];

  const raw = await readTextFile(FILE_PATH, { baseDir: BaseDirectory.AppData });
  const parsed = JSON.parse(raw);

  if (!Array.isArray(parsed?.tasks)) return [];

  return parsed.tasks.map((t: any) => ({
    ...t,
    created_at: new Date(t?.created_at ?? Date.now()),
  })) as Task[];
}

export async function saveToJson(tasks: Task[]) {
  const payload = JSON.stringify({
    version: 1,
    lastUpdated: Date.now(),
    tasks,
  });

  await writeTextFile(FILE_PATH, payload, { baseDir: BaseDirectory.AppData });
}
