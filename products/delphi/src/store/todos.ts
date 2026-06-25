import { defineStore } from "pinia";

import { computed, ref } from "vue";

import type {
  Area,
  Heading,
  Project,
  RecurrenceData,
  SmartList,
  Tag,
  TodoItem,
} from "@/types/task";

import {
  addChecklistItem as addChecklist,
  createTodoItem,
  markCancelled as markTodoCancelled,
  markCompleted as markTodoCompleted,
  markIncomplete as markTodoIncomplete,
  removeChecklistItem as removeChecklist,
  reorderChecklistItems as reorderChecklist,
  restoreFromTrash as restoreTodoItem,
  toggleChecklistItem as toggleChecklist,
  moveToTrash as trashTodoItem,
} from "@/models/todoItem";

import { countAll, filterTodos } from "@/services/filters/todoFilterService";

import {
  createNextRecurrence,
  duplicateTodo as duplicateTodoItem,
} from "@/services/recurrence/recurrence";

import type { CreateTodoParams } from "@/models/todoItem";

import { deleteTodoFromArk, persistTodoToArk } from "./todoArkPersistence";
import { createTodoReferenceActions } from "./todoReferenceActions";

// ---------------------------------------------------------------------------

// Helper: update a single todo in-place

// ---------------------------------------------------------------------------

function mapTodo(
  todos: TodoItem[],

  id: string,

  fn: (t: TodoItem) => TodoItem,
): TodoItem[] {
  return todos.map((t) => (t.id === id ? fn(t) : t));
}

// ---------------------------------------------------------------------------

// Store

// ---------------------------------------------------------------------------

export const useTodoStore = defineStore("todos", () => {
  // ---- State ----

  const todos = ref<TodoItem[]>([]);

  const projects = ref<Project[]>([]);

  const areas = ref<Area[]>([]);

  const tags = ref<Tag[]>([]);

  const headings = ref<Heading[]>([]);

  const activeSmartList = ref<SmartList | null>(null);

  const hydrated = ref(false);

  const {
    addProject,
    updateProject,
    removeProject,
    upsertProject,
    addArea,
    updateArea,
    removeArea,
    addTag,
    updateTag,
    removeTag,
    addHeading,
    updateHeading,
    removeHeading,
  } = createTodoReferenceActions({ todos, projects, areas, tags, headings });

  // ---- Bulk setters (for sync / hydration) ----

  function setTodos(value: TodoItem[]) {
    todos.value = value;
  }

  function setProjects(value: Project[]) {
    projects.value = value;
  }

  function setAreas(value: Area[]) {
    areas.value = value;
  }

  function setTags(value: Tag[]) {
    tags.value = value;
  }

  function setHeadings(value: Heading[]) {
    headings.value = value;
  }

  function setHydrated(v: boolean) {
    hydrated.value = v;
  }

  // ---- Navigation ----

  function setActiveSmartList(list: SmartList | null) {
    activeSmartList.value = list;
  }

  // ---- Derived ----

  function filteredTodos(list: SmartList): TodoItem[] {
    return filterTodos(list, todos.value);
  }

  const smartListCounts = computed(() => countAll(todos.value));

  function todosForProject(projectId: string): TodoItem[] {
    return todos.value

      .filter((t) => t.projectId === projectId && !t.isTrashed)

      .toSorted((a, b) => a.sortOrder - b.sortOrder);
  }

  function headingsForProject(projectId: string): Heading[] {
    return headings.value

      .filter((h) => h.projectId === projectId)

      .toSorted((a, b) => a.sortOrder - b.sortOrder);
  }

  // ---- Todo CRUD ----

  function addTodo(params: CreateTodoParams): TodoItem {
    const todo = createTodoItem(params);

    todos.value = [todo, ...todos.value];

    persistTodoToArk(todo);

    return todo;
  }

  function updateTodo(id: string, patch: Partial<TodoItem>) {
    todos.value = mapTodo(todos.value, id, (t) => ({ ...t, ...patch }));

    const updated = todos.value.find((t) => t.id === id);

    if (updated) {
      persistTodoToArk(updated);
    }
  }

  function removeTodo(id: string) {
    todos.value = todos.value.filter((t) => t.id !== id);

    deleteTodoFromArk(id);
  }

  /** Remove from local memory without sending to ARK (used for external ARK delete events). */

  function removeTodoLocal(id: string) {
    todos.value = todos.value.filter((t) => t.id !== id);
  }

  /** Remove project from local memory (legacy project model is renderer-local only). */

  function removeProjectLocal(id: string) {
    projects.value = projects.value.filter((p) => p.id !== id);

    // Unlink todos from removed project

    todos.value = todos.value.map((t) => (t.projectId === id ? { ...t, projectId: null } : t));
  }

  function upsertTodo(todo: TodoItem) {
    const exists = todos.value.some((t) => t.id === todo.id);

    todos.value = exists
      ? todos.value.map((t) => (t.id === todo.id ? todo : t))
      : [todo, ...todos.value];
  }

  function persistTodoLocallyAndInArk(todo: TodoItem) {
    persistTodoToArk(todo);
  }

  // ---- State transitions ----

  function completeTodo(id: string) {
    const todo = todos.value.find((t) => t.id === id);

    if (!todo) return;

    const completed = markTodoCompleted(todo);

    let newTodos = mapTodo(todos.value, id, () => completed);

    // If recurring, create next occurrence

    const next = createNextRecurrence(completed);

    if (next) {
      newTodos = [next, ...newTodos];

      persistTodoLocallyAndInArk(next);
    }

    todos.value = newTodos;

    persistTodoLocallyAndInArk(completed);
  }

  function incompleteTodo(id: string) {
    todos.value = mapTodo(todos.value, id, markTodoIncomplete);

    const updated = todos.value.find((t) => t.id === id);

    if (updated) {
      persistTodoLocallyAndInArk(updated);
    }
  }

  function cancelTodo(id: string) {
    todos.value = mapTodo(todos.value, id, markTodoCancelled);

    const updated = todos.value.find((t) => t.id === id);

    if (updated) {
      persistTodoLocallyAndInArk(updated);
    }
  }

  function trashTodo(id: string) {
    todos.value = mapTodo(todos.value, id, trashTodoItem);

    const updated = todos.value.find((t) => t.id === id);

    if (updated) {
      persistTodoLocallyAndInArk(updated);
    }
  }

  function restoreTodo(id: string) {
    todos.value = mapTodo(todos.value, id, restoreTodoItem);

    const updated = todos.value.find((t) => t.id === id);

    if (updated) {
      persistTodoLocallyAndInArk(updated);
    }
  }

  function duplicateTodo(id: string): TodoItem | null {
    const todo = todos.value.find((t) => t.id === id);

    if (!todo) return null;

    const copy = duplicateTodoItem(todo);

    todos.value = [copy, ...todos.value];
    persistTodoLocallyAndInArk(copy);

    return copy;
  }

  // ---- Checklist ----

  function addChecklistItem(todoId: string, title: string) {
    todos.value = mapTodo(todos.value, todoId, (t) => addChecklist(t, title));
    const updated = todos.value.find((t) => t.id === todoId);
    if (updated) {
      persistTodoLocallyAndInArk(updated);
    }
  }

  function toggleChecklistItem(todoId: string, itemId: string) {
    todos.value = mapTodo(todos.value, todoId, (t) => toggleChecklist(t, itemId));
    const updated = todos.value.find((t) => t.id === todoId);
    if (updated) {
      persistTodoLocallyAndInArk(updated);
    }
  }

  function removeChecklistItem(todoId: string, itemId: string) {
    todos.value = mapTodo(todos.value, todoId, (t) => removeChecklist(t, itemId));
    const updated = todos.value.find((t) => t.id === todoId);
    if (updated) {
      persistTodoLocallyAndInArk(updated);
    }
  }

  function reorderChecklistItems(todoId: string, orderedIds: string[]) {
    todos.value = mapTodo(todos.value, todoId, (t) => reorderChecklist(t, orderedIds));
    const updated = todos.value.find((t) => t.id === todoId);
    if (updated) {
      persistTodoLocallyAndInArk(updated);
    }
  }

  // ---- Recurrence ----

  function setRecurrence(todoId: string, rule: RecurrenceData | null) {
    todos.value = mapTodo(todos.value, todoId, (t) => ({
      ...t,

      recurrenceRule: rule,
    }));
    const updated = todos.value.find((t) => t.id === todoId);
    if (updated) {
      persistTodoLocallyAndInArk(updated);
    }
  }

  // ---- Tags on todos ----

  function addTagToTodo(todoId: string, tagId: string) {
    todos.value = mapTodo(todos.value, todoId, (t) =>
      t.tagIds.includes(tagId) ? t : { ...t, tagIds: [...t.tagIds, tagId] },
    );
    const updated = todos.value.find((t) => t.id === todoId);
    if (updated) {
      persistTodoLocallyAndInArk(updated);
    }
  }

  function removeTagFromTodo(todoId: string, tagId: string) {
    todos.value = mapTodo(todos.value, todoId, (t) => ({
      ...t,

      tagIds: t.tagIds.filter((id) => id !== tagId),
    }));
    const updated = todos.value.find((t) => t.id === todoId);
    if (updated) {
      persistTodoLocallyAndInArk(updated);
    }
  }

  async function emptyTrash() {
    const trashedTodos = todos.value.filter((t) => t.isTrashed);
    for (const t of trashedTodos) {
      deleteTodoFromArk(t.id);
    }

    // Remove trashed todos from store
    todos.value = todos.value.filter((t) => !t.isTrashed);

    await Promise.resolve();
  }

  return {
    // State

    todos,

    projects,

    areas,

    tags,

    headings,

    activeSmartList,

    hydrated,

    // Bulk setters

    setTodos,

    setProjects,

    setAreas,

    setTags,

    setHeadings,

    setHydrated,

    // Navigation

    setActiveSmartList,

    // Derived

    filteredTodos,

    smartListCounts,

    todosForProject,

    headingsForProject,

    // Todo CRUD

    addTodo,

    updateTodo,

    removeTodo,

    removeTodoLocal,

    removeProjectLocal,

    upsertTodo,

    // State transitions

    completeTodo,

    incompleteTodo,

    cancelTodo,

    trashTodo,

    restoreTodo,

    emptyTrash,

    duplicateTodo,

    // Checklist

    addChecklistItem,

    toggleChecklistItem,

    removeChecklistItem,

    reorderChecklistItems,

    // Recurrence

    setRecurrence,

    // Tags on todos

    addTagToTodo,

    removeTagFromTodo,

    // Project CRUD

    addProject,

    updateProject,

    removeProject,

    upsertProject,

    // Area CRUD

    addArea,

    updateArea,

    removeArea,

    // Tag CRUD

    addTag,

    updateTag,

    removeTag,

    // Heading CRUD

    addHeading,

    updateHeading,

    removeHeading,
  };
});
