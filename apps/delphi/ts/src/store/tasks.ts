import { create } from 'zustand';
import { api } from '@/services/api/client';
import { fromTaskDto } from '@/services/api/tasks';
import type { Task } from '@/types/task';

type TaskStore = {
  tasks: Task[];
  setTasks: (tasks: Task[]) => void;
  addTask: (title: string) => Promise<Task>;
  removeTask: (task: Task) => Promise<void>;
  removeAllTasks: () => void;
  completeTask: (task: Task) => Promise<void>;
  editTask: (task: Task) => Promise<void>;
  upsertTask: (task: Task) => void;
  removeTaskById: (id: string) => void;
  hydrated: boolean;
  setHydrated: (value: boolean) => void;
};

const useTask = create<TaskStore>((set) => ({
  tasks: [],
  setTasks: (tasks) => set({ tasks }),
  addTask: async (title) => {
    const taskTitle = title.trim() || 'New task';
    const created = await api.createTask(taskTitle);
    const createdTask = fromTaskDto(created);
    set((state) => ({
      tasks: state.tasks.some((item) => item.id === createdTask.id)
        ? state.tasks.map((item) => (item.id === createdTask.id ? createdTask : item))
        : [createdTask, ...state.tasks],
    }));
    return createdTask;
  },
  removeTask: async (task) => {
    await api.deleteTask(task.id);
    set((state) => ({
      tasks: state.tasks.filter((item) => item.id !== task.id),
    }));
  },
  removeAllTasks: () => set({ tasks: [] }),
  completeTask: async (task) => {
    const updated = await api.updateTask(task.id, {
      completed: !task.completed,
    });
    set((state) => ({
      tasks: state.tasks.map((item) =>
        item.id === task.id ? fromTaskDto(updated) : item,
      ),
    }));
  },
  editTask: async (task) => {
    const taskTitle = task.title.trim() || 'New task';
    const updated = await api.updateTask(task.id, {
      title: taskTitle,
    });
    set((state) => ({
      tasks: state.tasks.map((item) =>
        item.id === task.id ? fromTaskDto(updated) : item,
      ),
    }));
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
