import { create } from "zustand";
import type {
  TodoItem,
  Project,
  Area,
  Tag,
  Heading,
  SmartList,
} from "@/types/task";
import {
  createTodoItem,
  markCompleted as markTodoCompleted,
  markIncomplete as markTodoIncomplete,
  markCancelled as markTodoCancelled,
  moveToTrash as trashTodo,
  restoreFromTrash as restoreTodo,
  addChecklistItem as addChecklist,
  toggleChecklistItem as toggleChecklist,
  removeChecklistItem as removeChecklist,
} from "@/models/todoItem";
import { createProject } from "@/models/project";
import { createArea } from "@/models/area";
import { createTag } from "@/models/tag";
import { createHeading } from "@/models/heading";
import { filterTodos, countAll } from "@/services/filters/todoFilterService";
import type { CreateTodoParams } from "@/models/todoItem";
import type { CreateProjectParams } from "@/models/project";
import {
  loadTodos,
  saveTodos,
  loadProjects,
  saveProjects,
} from "@/db/storage";

// ---------------------------------------------------------------------------
// Store shape
// ---------------------------------------------------------------------------

type TodoStore = {
  // Data
  todos: TodoItem[];
  projects: Project[];
  areas: Area[];
  tags: Tag[];
  headings: Heading[];

  // Hydration
  hydrated: boolean;
  hydrate: () => Promise<void>;

  // Bulk setters (for sync)
  setTodos: (todos: TodoItem[]) => void;
  setProjects: (projects: Project[]) => void;

  // Derived
  filteredTodos: (list: SmartList) => TodoItem[];
  smartListCounts: () => Record<SmartList, number>;

  // Todo CRUD
  addTodo: (params: CreateTodoParams) => TodoItem;
  updateTodo: (id: string, patch: Partial<TodoItem>) => void;
  removeTodo: (id: string) => void;
  upsertTodo: (todo: TodoItem) => void;

  // Todo state transitions
  completeTodo: (id: string) => void;
  incompleteTodo: (id: string) => void;
  cancelTodo: (id: string) => void;
  trashTodo: (id: string) => void;
  restoreTodo: (id: string) => void;

  // Checklist
  addChecklistItem: (todoId: string, title: string) => void;
  toggleChecklistItem: (todoId: string, itemId: string) => void;
  removeChecklistItem: (todoId: string, itemId: string) => void;

  // Tags on todos
  addTagToTodo: (todoId: string, tagId: string) => void;
  removeTagFromTodo: (todoId: string, tagId: string) => void;

  // Project CRUD
  addProject: (params: CreateProjectParams) => Project;
  updateProject: (id: string, patch: Partial<Project>) => void;
  removeProject: (id: string) => void;
  upsertProject: (project: Project) => void;

  // Area CRUD
  addArea: (title: string) => Area;
  removeArea: (id: string) => void;

  // Tag CRUD
  addTag: (title: string, color?: string) => Tag;
  removeTag: (id: string) => void;

  // Heading CRUD
  addHeading: (title: string, projectId?: string | null) => Heading;
  removeHeading: (id: string) => void;
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function mapTodo(
  todos: TodoItem[],
  id: string,
  fn: (t: TodoItem) => TodoItem,
): TodoItem[] {
  return todos.map((t) => (t.id === id ? fn(t) : t));
}

/** Persist todos to SQLite (fire-and-forget). */
function persistTodos(todos: TodoItem[]) {
  saveTodos(todos).catch((e) =>
    console.warn("[Store] Failed to persist todos:", e),
  );
}

function persistProjects(projects: Project[]) {
  saveProjects(projects).catch((e) =>
    console.warn("[Store] Failed to persist projects:", e),
  );
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

const useTodoStore = create<TodoStore>((set, get) => ({
  todos: [],
  projects: [],
  areas: [],
  tags: [],
  headings: [],

  hydrated: false,

  hydrate: async () => {
    try {
      const [todos, projects] = await Promise.all([
        loadTodos(),
        loadProjects(),
      ]);
      set({ todos, projects, hydrated: true });
    } catch (e) {
      console.warn("[Store] Hydration failed:", e);
      set({ hydrated: true });
    }
  },

  // Bulk setters
  setTodos: (todos) => {
    set({ todos });
    persistTodos(todos);
  },
  setProjects: (projects) => {
    set({ projects });
    persistProjects(projects);
  },

  // Derived
  filteredTodos: (list) => filterTodos(list, get().todos),
  smartListCounts: () => countAll(get().todos),

  // ----- Todo CRUD -----

  addTodo: (params) => {
    const todo = createTodoItem(params);
    const todos = [todo, ...get().todos];
    set({ todos });
    persistTodos(todos);
    return todo;
  },

  updateTodo: (id, patch) => {
    const todos = mapTodo(get().todos, id, (t) => ({ ...t, ...patch }));
    set({ todos });
    persistTodos(todos);
  },

  removeTodo: (id) => {
    const todos = get().todos.filter((t) => t.id !== id);
    set({ todos });
    persistTodos(todos);
  },

  upsertTodo: (todo) => {
    const state = get();
    const exists = state.todos.some((t) => t.id === todo.id);
    const todos = exists
      ? state.todos.map((t) => (t.id === todo.id ? todo : t))
      : [todo, ...state.todos];
    set({ todos });
    persistTodos(todos);
  },

  // ----- State transitions -----

  completeTodo: (id) => {
    const todos = mapTodo(get().todos, id, markTodoCompleted);
    set({ todos });
    persistTodos(todos);
  },

  incompleteTodo: (id) => {
    const todos = mapTodo(get().todos, id, markTodoIncomplete);
    set({ todos });
    persistTodos(todos);
  },

  cancelTodo: (id) => {
    const todos = mapTodo(get().todos, id, markTodoCancelled);
    set({ todos });
    persistTodos(todos);
  },

  trashTodo: (id) => {
    const todos = mapTodo(get().todos, id, trashTodo);
    set({ todos });
    persistTodos(todos);
  },

  restoreTodo: (id) => {
    const todos = mapTodo(get().todos, id, restoreTodo);
    set({ todos });
    persistTodos(todos);
  },

  // ----- Checklist -----

  addChecklistItem: (todoId, title) => {
    const todos = mapTodo(get().todos, todoId, (t) => addChecklist(t, title));
    set({ todos });
    persistTodos(todos);
  },

  toggleChecklistItem: (todoId, itemId) => {
    const todos = mapTodo(get().todos, todoId, (t) =>
      toggleChecklist(t, itemId),
    );
    set({ todos });
    persistTodos(todos);
  },

  removeChecklistItem: (todoId, itemId) => {
    const todos = mapTodo(get().todos, todoId, (t) =>
      removeChecklist(t, itemId),
    );
    set({ todos });
    persistTodos(todos);
  },

  // ----- Tags on todos -----

  addTagToTodo: (todoId, tagId) => {
    const todos = mapTodo(get().todos, todoId, (t) =>
      t.tagIds.includes(tagId) ? t : { ...t, tagIds: [...t.tagIds, tagId] },
    );
    set({ todos });
    persistTodos(todos);
  },

  removeTagFromTodo: (todoId, tagId) => {
    const todos = mapTodo(get().todos, todoId, (t) => ({
      ...t,
      tagIds: t.tagIds.filter((id) => id !== tagId),
    }));
    set({ todos });
    persistTodos(todos);
  },

  // ----- Project CRUD -----

  addProject: (params) => {
    const project = createProject(params);
    const projects = [project, ...get().projects];
    set({ projects });
    persistProjects(projects);
    return project;
  },

  updateProject: (id, patch) => {
    const projects = get().projects.map((p) =>
      p.id === id ? { ...p, ...patch } : p,
    );
    set({ projects });
    persistProjects(projects);
  },

  removeProject: (id) => {
    const projects = get().projects.filter((p) => p.id !== id);
    const todos = get().todos.map((t) =>
      t.projectId === id ? { ...t, projectId: null } : t,
    );
    set({ projects, todos });
    persistProjects(projects);
    persistTodos(todos);
  },

  upsertProject: (project) => {
    const state = get();
    const exists = state.projects.some((p) => p.id === project.id);
    const projects = exists
      ? state.projects.map((p) => (p.id === project.id ? project : p))
      : [project, ...state.projects];
    set({ projects });
    persistProjects(projects);
  },

  // ----- Area CRUD -----

  addArea: (title) => {
    const area = createArea(title);
    set((s) => ({ areas: [area, ...s.areas] }));
    return area;
  },

  removeArea: (id) =>
    set((s) => ({
      areas: s.areas.filter((a) => a.id !== id),
      projects: s.projects.map((p) =>
        p.areaId === id ? { ...p, areaId: null } : p,
      ),
      todos: s.todos.map((t) => (t.areaId === id ? { ...t, areaId: null } : t)),
    })),

  // ----- Tag CRUD -----

  addTag: (title, color) => {
    const tag = createTag(title, color);
    set((s) => ({ tags: [tag, ...s.tags] }));
    return tag;
  },

  removeTag: (id) =>
    set((s) => ({
      tags: s.tags.filter((t) => t.id !== id),
      todos: s.todos.map((t) =>
        t.tagIds.includes(id)
          ? { ...t, tagIds: t.tagIds.filter((tid) => tid !== id) }
          : t,
      ),
    })),

  // ----- Heading CRUD -----

  addHeading: (title, projectId) => {
    const heading = createHeading(title, projectId);
    set((s) => ({ headings: [heading, ...s.headings] }));
    return heading;
  },

  removeHeading: (id) =>
    set((s) => ({
      headings: s.headings.filter((h) => h.id !== id),
      todos: s.todos.map((t) =>
        t.headingId === id ? { ...t, headingId: null } : t,
      ),
    })),
}));

export default useTodoStore;
