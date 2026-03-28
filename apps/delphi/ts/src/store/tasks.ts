import { create } from 'zustand';
import { arkSync, taskToArkChange } from '@/services/sync/ark-client';
import type { Task } from '@/types/task';

type TaskStore = {
  tasks: Task[];
  setTasks: (tasks: Task[]) => void;
  addTask: (title: string) => Task;
  removeTask: (task: Task) => void;
  removeAllTasks: () => void;
  completeTask: (task: Task) => void;
  editTask: (task: Task) => void;
  upsertTask: (task: Task) => void;
  removeTaskById: (id: string) => void;
  hydrated: boolean;
  setHydrated: (value: boolean) => void;
};

const useTask = create<TaskStore>((set) => ({
  tasks: [],
  setTasks: (tasks) => set({ tasks }),
  addTask: (title) => {
    const taskTitle = title.trim() || 'New task';
    const now = new Date();
    const task: Task = {
      id: crypto.randomUUID(),
      title: taskTitle,
      description: null,
      completed: false,
      priority: 0,
      due_date: null,
      list_id: null,
      created_at: now,
      updated_at: now,
    };

    set((state) => ({ tasks: [task, ...state.tasks] }));
    arkSync.sendChange(taskToArkChange(task, 'create'));
    return task;
  },
  removeTask: (task) => {
    set((state) => ({
      tasks: state.tasks.filter((item) => item.id !== task.id),
    }));
    arkSync.sendChange(taskToArkChange(task, 'delete'));
  },
  removeAllTasks: () => set({ tasks: [] }),
  completeTask: (task) => {
    const updated: Task = {
      ...task,
      completed: !task.completed,
      updated_at: new Date(),
    };
    set((state) => ({
      tasks: state.tasks.map((item) =>
        item.id === task.id ? updated : item,
      ),
    }));
    arkSync.sendChange(taskToArkChange(updated, 'update'));
  },
  editTask: (task) => {
    const updated: Task = {
      ...task,
      title: task.title.trim() || 'New task',
      updated_at: new Date(),
    };
    set((state) => ({
      tasks: state.tasks.map((item) =>
        item.id === task.id ? updated : item,
      ),
    }));
    arkSync.sendChange(taskToArkChange(updated, 'update'));
  },
  upsertTask: (task) =>
    set((state) => {
      const exists = state.tasks.some((item) => item.id === task.id);
      if (exists) {
        return {
          tasks: state.tasks.map((item) => (item.id === task.id ? task : item)),
        };
      }

      return { tasks: [task, ...state.tasks] };
    }),
  removeTaskById: (id) =>
    set((state) => ({
      tasks: state.tasks.filter((item) => item.id !== id),
    })),
  hydrated: false,
  setHydrated: (value) => set({ hydrated: value }),
}));

export default useTask;
