import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type {
  TodoItem,
  Project,
  Area,
  Tag,
  Heading,
  SmartList,
  RecurrenceData,
} from "@/types/task";
import {
  createTodoItem,
  markCompleted as markTodoCompleted,
  markIncomplete as markTodoIncomplete,
  markCancelled as markTodoCancelled,
  moveToTrash as trashTodoItem,
  restoreFromTrash as restoreTodoItem,
  addChecklistItem as addChecklist,
  toggleChecklistItem as toggleChecklist,
  removeChecklistItem as removeChecklist,
  reorderChecklistItems as reorderChecklist,
} from "@/models/todoItem";
import { createProject } from "@/models/project";
import { createArea } from "@/models/area";
import { createTag } from "@/models/tag";
import { createHeading } from "@/models/heading";
import { filterTodos, countAll } from "@/services/filters/todoFilterService";
import {
  createNextRecurrence,
  duplicateTodo as duplicateTodoItem,
} from "@/services/recurrence/recurrence";
import type { CreateTodoParams } from "@/models/todoItem";
import type { CreateProjectParams } from "@/models/project";

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
      .sort((a, b) => a.sortOrder - b.sortOrder);
  }

  function headingsForProject(projectId: string): Heading[] {
    return headings.value
      .filter((h) => h.projectId === projectId)
      .sort((a, b) => a.sortOrder - b.sortOrder);
  }

  // ---- Todo CRUD ----

  function addTodo(params: CreateTodoParams): TodoItem {
    const todo = createTodoItem(params);
    todos.value = [todo, ...todos.value];
    return todo;
  }

  function updateTodo(id: string, patch: Partial<TodoItem>) {
    todos.value = mapTodo(todos.value, id, (t) => ({ ...t, ...patch }));
  }

  function removeTodo(id: string) {
    todos.value = todos.value.filter((t) => t.id !== id);
  }

  function upsertTodo(todo: TodoItem) {
    const exists = todos.value.some((t) => t.id === todo.id);
    todos.value = exists
      ? todos.value.map((t) => (t.id === todo.id ? todo : t))
      : [todo, ...todos.value];
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
    }

    todos.value = newTodos;
  }

  function incompleteTodo(id: string) {
    todos.value = mapTodo(todos.value, id, markTodoIncomplete);
  }

  function cancelTodo(id: string) {
    todos.value = mapTodo(todos.value, id, markTodoCancelled);
  }

  function trashTodo(id: string) {
    todos.value = mapTodo(todos.value, id, trashTodoItem);
  }

  function restoreTodo(id: string) {
    todos.value = mapTodo(todos.value, id, restoreTodoItem);
  }

  function duplicateTodo(id: string): TodoItem | null {
    const todo = todos.value.find((t) => t.id === id);
    if (!todo) return null;
    const copy = duplicateTodoItem(todo);
    todos.value = [copy, ...todos.value];
    return copy;
  }

  // ---- Checklist ----

  function addChecklistItem(todoId: string, title: string) {
    todos.value = mapTodo(todos.value, todoId, (t) => addChecklist(t, title));
  }

  function toggleChecklistItem(todoId: string, itemId: string) {
    todos.value = mapTodo(todos.value, todoId, (t) =>
      toggleChecklist(t, itemId),
    );
  }

  function removeChecklistItem(todoId: string, itemId: string) {
    todos.value = mapTodo(todos.value, todoId, (t) =>
      removeChecklist(t, itemId),
    );
  }

  function reorderChecklistItems(todoId: string, orderedIds: string[]) {
    todos.value = mapTodo(todos.value, todoId, (t) =>
      reorderChecklist(t, orderedIds),
    );
  }

  // ---- Recurrence ----

  function setRecurrence(todoId: string, rule: RecurrenceData | null) {
    todos.value = mapTodo(todos.value, todoId, (t) => ({
      ...t,
      recurrenceRule: rule,
    }));
  }

  // ---- Tags on todos ----

  function addTagToTodo(todoId: string, tagId: string) {
    todos.value = mapTodo(todos.value, todoId, (t) =>
      t.tagIds.includes(tagId) ? t : { ...t, tagIds: [...t.tagIds, tagId] },
    );
  }

  function removeTagFromTodo(todoId: string, tagId: string) {
    todos.value = mapTodo(todos.value, todoId, (t) => ({
      ...t,
      tagIds: t.tagIds.filter((id) => id !== tagId),
    }));
  }

  // ---- Project CRUD ----

  function addProject(params: CreateProjectParams): Project {
    const project = createProject(params);
    projects.value = [project, ...projects.value];
    return project;
  }

  function updateProject(id: string, patch: Partial<Project>) {
    projects.value = projects.value.map((p) =>
      p.id === id ? { ...p, ...patch } : p,
    );
  }

  function removeProject(id: string) {
    projects.value = projects.value.filter((p) => p.id !== id);
    // Unlink todos from removed project
    todos.value = todos.value.map((t) =>
      t.projectId === id ? { ...t, projectId: null } : t,
    );
  }

  function upsertProject(project: Project) {
    const exists = projects.value.some((p) => p.id === project.id);
    projects.value = exists
      ? projects.value.map((p) => (p.id === project.id ? project : p))
      : [project, ...projects.value];
  }

  // ---- Area CRUD ----

  function addArea(title: string): Area {
    const area = createArea(title);
    areas.value = [area, ...areas.value];
    return area;
  }

  function updateArea(id: string, patch: Partial<Area>) {
    areas.value = areas.value.map((a) =>
      a.id === id ? { ...a, ...patch } : a,
    );
  }

  function removeArea(id: string) {
    areas.value = areas.value.filter((a) => a.id !== id);
    // Unlink projects & todos from removed area
    projects.value = projects.value.map((p) =>
      p.areaId === id ? { ...p, areaId: null } : p,
    );
    todos.value = todos.value.map((t) =>
      t.areaId === id ? { ...t, areaId: null } : t,
    );
  }

  // ---- Tag CRUD ----

  function addTag(
    title: string,
    color?: string,
    shortcut?: string | null,
  ): Tag {
    const tag = createTag(title, color, shortcut);
    tags.value = [tag, ...tags.value];
    return tag;
  }

  function updateTag(id: string, patch: Partial<Tag>) {
    tags.value = tags.value.map((t) => (t.id === id ? { ...t, ...patch } : t));
  }

  function removeTag(id: string) {
    tags.value = tags.value.filter((t) => t.id !== id);
    // Remove tag from all todos
    todos.value = todos.value.map((t) =>
      t.tagIds.includes(id)
        ? { ...t, tagIds: t.tagIds.filter((tid) => tid !== id) }
        : t,
    );
  }

  // ---- Heading CRUD ----

  function addHeading(title: string, projectId?: string | null): Heading {
    const heading = createHeading(title, projectId);
    headings.value = [heading, ...headings.value];
    return heading;
  }

  function updateHeading(id: string, patch: Partial<Heading>) {
    headings.value = headings.value.map((h) =>
      h.id === id ? { ...h, ...patch } : h,
    );
  }

  function removeHeading(id: string) {
    headings.value = headings.value.filter((h) => h.id !== id);
    // Clear headingId from todos that referenced it
    todos.value = todos.value.map((t) =>
      t.headingId === id ? { ...t, headingId: null } : t,
    );
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
    upsertTodo,

    // State transitions
    completeTodo,
    incompleteTodo,
    cancelTodo,
    trashTodo,
    restoreTodo,
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

export default useTodoStore;
