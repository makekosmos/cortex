import { defineStore } from "pinia";

import { ref } from "vue";

import type { Task } from "@/types/task";

export const useTaskStore = defineStore("tasks", () => {
  const tasks = ref<Task[]>([]);

  const hydrated = ref(false);

  function setTasks(value: Task[]) {
    tasks.value = value;
  }

  function setHydrated(value: boolean) {
    hydrated.value = value;
  }

  function addTask(title: string): Task {
    const taskTitle = title.trim() || "New task";

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

    tasks.value = [task, ...tasks.value];

    return task;
  }

  function removeTask(task: Task) {
    tasks.value = tasks.value.filter((item) => item.id !== task.id);
  }

  function removeAllTasks() {
    tasks.value = [];
  }

  function completeTask(task: Task) {
    const updated: Task = {
      ...task,

      completed: !task.completed,

      updated_at: new Date(),
    };

    tasks.value = tasks.value.map((item) => (item.id === task.id ? updated : item));
  }

  function editTask(task: Task) {
    const updated: Task = {
      ...task,

      title: task.title.trim() || "New task",

      updated_at: new Date(),
    };

    tasks.value = tasks.value.map((item) => (item.id === task.id ? updated : item));
  }

  function upsertTask(task: Task) {
    const exists = tasks.value.some((item) => item.id === task.id);

    tasks.value = exists
      ? tasks.value.map((item) => (item.id === task.id ? task : item))
      : [task, ...tasks.value];
  }

  function removeTaskById(id: string) {
    tasks.value = tasks.value.filter((item) => item.id !== id);
  }

  return {
    tasks,

    hydrated,

    setTasks,

    setHydrated,

    addTask,

    removeTask,

    removeAllTasks,

    completeTask,

    editTask,

    upsertTask,

    removeTaskById,
  };
});

export default useTaskStore;
