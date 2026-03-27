import { loadTasksFromWebStorage, saveTasksToWebStorage } from '@/services/storage/tasks.web';
import type { Task } from '@/types/task';

const TASKS_KEY = 'todofus.tasks';

describe('web task storage', () => {
  beforeEach(() => {
    localStorage.clear();
    delete (window as Window & { electronAPI?: unknown }).electronAPI;
  });

  it('round-trips tasks in browser storage', () => {
    const now = new Date('2026-02-20T12:00:00.000Z');
    const tasks: Task[] = [
      {
        id: '1',
        title: 'Test task',
        description: null,
        completed: false,
        created_at: now,
        updated_at: now,
      },
    ];

    saveTasksToWebStorage(tasks);
    const loaded = loadTasksFromWebStorage();

    expect(loaded).toHaveLength(1);
    expect(loaded[0].id).toBe('1');
    expect(loaded[0].created_at).toBeInstanceOf(Date);
  });

  it('returns empty array on invalid JSON', () => {
    localStorage.setItem(TASKS_KEY, '{not-json');
    expect(loadTasksFromWebStorage()).toEqual([]);
  });

  it('does not write browser cache in electron runtime', () => {
    (window as Window & { electronAPI?: unknown }).electronAPI = {} as ElectronAPI;

    saveTasksToWebStorage([
      {
        id: '1',
        title: 'Should not persist',
        completed: false,
        created_at: new Date(),
      },
    ]);

    expect(localStorage.getItem(TASKS_KEY)).toBeNull();
  });
});
