import { defineStore } from "pinia";

import { ref } from "vue";

import { arkSync, taskToArkChange } from "@/services/sync/ark-client";

import { broadcastToPeers } from "@/services/sync/peer-bridge";

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

    const change = taskToArkChange(task, "create");

    arkSync.sendChange(change);

    broadcastToPeers(change);

    return task;
  }

  function removeTask(task: Task) {
    tasks.value = tasks.value.filter((item) => item.id !== task.id);

    const change = taskToArkChange(task, "delete");

    arkSync.sendChange(change);

    broadcastToPeers(change);
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

    tasks.value = tasks.value.map((item) =>
      item.id === task.id ? updated : item,
    );

    const change = taskToArkChange(updated, "update");

    arkSync.sendChange(change);

    broadcastToPeers(change);
  }

  function editTask(task: Task) {
    const updated: Task = {
      ...task,

      title: task.title.trim() || "New task",

      updated_at: new Date(),
    };

    tasks.value = tasks.value.map((item) =>
      item.id === task.id ? updated : item,
    );

    const change = taskToArkChange(updated, "update");

    arkSync.sendChange(change);

    broadcastToPeers(change);
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
