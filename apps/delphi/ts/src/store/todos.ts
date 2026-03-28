import { create } from "zustand";
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
  moveToTrash as trashTodo,
  restoreFromTrash as restoreTodo,
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
  duplicateTodo,
} from "@/services/recurrence/recurrence";
import type { CreateTodoParams } from "@/models/todoItem";
import type { CreateProjectParams } from "@/models/project";

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
  activeSmartList: SmartList | null;

  // Hydration
  hydrated: boolean;
  setHydrated: (v: boolean) => void;

  // Bulk setters (for sync / hydration)
  setTodos: (todos: TodoItem[]) => void;
  setProjects: (projects: Project[]) => void;
  setAreas: (areas: Area[]) => void;
  setTags: (tags: Tag[]) => void;
  setHeadings: (headings: Heading[]) => void;

  // Navigation
  setActiveSmartList: (list: SmartList | null) => void;

  // Derived
  filteredTodos: (list: SmartList) => TodoItem[];
  smartListCounts: () => Record<SmartList, number>;
  todosForProject: (projectId: string) => TodoItem[];
  headingsForProject: (projectId: string) => Heading[];

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
  duplicateTodo: (id: string) => TodoItem | null;

  // Checklist
  addChecklistItem: (todoId: string, title: string) => void;
  toggleChecklistItem: (todoId: string, itemId: string) => void;
  removeChecklistItem: (todoId: string, itemId: string) => void;
  reorderChecklistItems: (todoId: string, orderedIds: string[]) => void;

  // Recurrence
  setRecurrence: (todoId: string, rule: RecurrenceData | null) => void;

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
  updateArea: (id: string, patch: Partial<Area>) => void;
  removeArea: (id: string) => void;

  // Tag CRUD
  addTag: (title: string, color?: string, shortcut?: string | null) => Tag;
  updateTag: (id: string, patch: Partial<Tag>) => void;
  removeTag: (id: string) => void;

  // Heading CRUD
  addHeading: (title: string, projectId?: string | null) => Heading;
  updateHeading: (id: string, patch: Partial<Heading>) => void;
  removeHeading: (id: string) => void;
};

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

const useTodoStore = create<TodoStore>((set, get) => ({
  // Data
  todos: [],
  projects: [],
  areas: [],
  tags: [],
  headings: [],
  activeSmartList: null,

  hydrated: false,
  setHydrated: (v) => set({ hydrated: v }),

  // Bulk setters
  setTodos: (todos) => set({ todos }),
  setProjects: (projects) => set({ projects }),
  setAreas: (areas) => set({ areas }),
  setTags: (tags) => set({ tags }),
  setHeadings: (headings) => set({ headings }),

  // Navigation
  setActiveSmartList: (list) => set({ activeSmartList: list }),

  // Derived (not stored — computed on call)
  filteredTodos: (list) => filterTodos(list, get().todos),
  smartListCounts: () => countAll(get().todos),
  todosForProject: (projectId) =>
    get()
      .todos.filter((t) => t.projectId === projectId && !t.isTrashed)
      .sort((a, b) => a.sortOrder - b.sortOrder),
  headingsForProject: (projectId) =>
    get()
      .headings.filter((h) => h.projectId === projectId)
      .sort((a, b) => a.sortOrder - b.sortOrder),

  // ----- Todo CRUD -----

  addTodo: (params) => {
    const todo = createTodoItem(params);
    set((s) => ({ todos: [todo, ...s.todos] }));
    return todo;
  },

  updateTodo: (id, patch) =>
    set((s) => ({
      todos: mapTodo(s.todos, id, (t) => ({ ...t, ...patch })),
    })),

  removeTodo: (id) =>
    set((s) => ({ todos: s.todos.filter((t) => t.id !== id) })),

  upsertTodo: (todo) =>
    set((s) => {
      const exists = s.todos.some((t) => t.id === todo.id);
      return {
        todos: exists
          ? s.todos.map((t) => (t.id === todo.id ? todo : t))
          : [todo, ...s.todos],
      };
    }),

  // ----- State transitions -----

  completeTodo: (id) => {
    const state = get();
    const todo = state.todos.find((t) => t.id === id);
    if (!todo) return;

    const completed = markTodoCompleted(todo);
    let newTodos = mapTodo(state.todos, id, () => completed);

    // If recurring, create next occurrence
    const next = createNextRecurrence(completed);
    if (next) {
      newTodos = [next, ...newTodos];
    }

    set({ todos: newTodos });
  },

  incompleteTodo: (id) =>
    set((s) => ({
      todos: mapTodo(s.todos, id, markTodoIncomplete),
    })),

  cancelTodo: (id) =>
    set((s) => ({
      todos: mapTodo(s.todos, id, markTodoCancelled),
    })),

  trashTodo: (id) =>
    set((s) => ({
      todos: mapTodo(s.todos, id, trashTodo),
    })),

  restoreTodo: (id) =>
    set((s) => ({
      todos: mapTodo(s.todos, id, restoreTodo),
    })),

  duplicateTodo: (id) => {
    const todo = get().todos.find((t) => t.id === id);
    if (!todo) return null;
    const copy = duplicateTodo(todo);
    set((s) => ({ todos: [copy, ...s.todos] }));
    return copy;
  },

  // ----- Checklist -----

  addChecklistItem: (todoId, title) =>
    set((s) => ({
      todos: mapTodo(s.todos, todoId, (t) => addChecklist(t, title)),
    })),

  toggleChecklistItem: (todoId, itemId) =>
    set((s) => ({
      todos: mapTodo(s.todos, todoId, (t) => toggleChecklist(t, itemId)),
    })),

  removeChecklistItem: (todoId, itemId) =>
    set((s) => ({
      todos: mapTodo(s.todos, todoId, (t) => removeChecklist(t, itemId)),
    })),

  reorderChecklistItems: (todoId, orderedIds) =>
    set((s) => ({
      todos: mapTodo(s.todos, todoId, (t) => reorderChecklist(t, orderedIds)),
    })),

  // ----- Recurrence -----

  setRecurrence: (todoId, rule) =>
    set((s) => ({
      todos: mapTodo(s.todos, todoId, (t) => ({ ...t, recurrenceRule: rule })),
    })),

  // ----- Tags on todos -----

  addTagToTodo: (todoId, tagId) =>
    set((s) => ({
      todos: mapTodo(s.todos, todoId, (t) =>
        t.tagIds.includes(tagId) ? t : { ...t, tagIds: [...t.tagIds, tagId] },
      ),
    })),

  removeTagFromTodo: (todoId, tagId) =>
    set((s) => ({
      todos: mapTodo(s.todos, todoId, (t) => ({
        ...t,
        tagIds: t.tagIds.filter((id) => id !== tagId),
      })),
    })),

  // ----- Project CRUD -----

  addProject: (params) => {
    const project = createProject(params);
    set((s) => ({ projects: [project, ...s.projects] }));
    return project;
  },

  updateProject: (id, patch) =>
    set((s) => ({
      projects: s.projects.map((p) => (p.id === id ? { ...p, ...patch } : p)),
    })),

  removeProject: (id) =>
    set((s) => ({
      projects: s.projects.filter((p) => p.id !== id),
      // Unlink todos from removed project
      todos: s.todos.map((t) =>
        t.projectId === id ? { ...t, projectId: null } : t,
      ),
    })),

  upsertProject: (project) =>
    set((s) => {
      const exists = s.projects.some((p) => p.id === project.id);
      return {
        projects: exists
          ? s.projects.map((p) => (p.id === project.id ? project : p))
          : [project, ...s.projects],
      };
    }),

  // ----- Area CRUD -----

  addArea: (title) => {
    const area = createArea(title);
    set((s) => ({ areas: [area, ...s.areas] }));
    return area;
  },

  updateArea: (id, patch) =>
    set((s) => ({
      areas: s.areas.map((a) => (a.id === id ? { ...a, ...patch } : a)),
    })),

  removeArea: (id) =>
    set((s) => ({
      areas: s.areas.filter((a) => a.id !== id),
      // Unlink projects & todos from removed area
      projects: s.projects.map((p) =>
        p.areaId === id ? { ...p, areaId: null } : p,
      ),
      todos: s.todos.map((t) => (t.areaId === id ? { ...t, areaId: null } : t)),
    })),

  // ----- Tag CRUD -----

  addTag: (title, color, shortcut) => {
    const tag = createTag(title, color, shortcut);
    set((s) => ({ tags: [tag, ...s.tags] }));
    return tag;
  },

  updateTag: (id, patch) =>
    set((s) => ({
      tags: s.tags.map((t) => (t.id === id ? { ...t, ...patch } : t)),
    })),

  removeTag: (id) =>
    set((s) => ({
      tags: s.tags.filter((t) => t.id !== id),
      // Remove tag from all todos
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

  updateHeading: (id, patch) =>
    set((s) => ({
      headings: s.headings.map((h) => (h.id === id ? { ...h, ...patch } : h)),
    })),

  removeHeading: (id) =>
    set((s) => ({
      headings: s.headings.filter((h) => h.id !== id),
      // Clear headingId from todos that referenced it
      todos: s.todos.map((t) =>
        t.headingId === id ? { ...t, headingId: null } : t,
      ),
    })),
}));

export default useTodoStore;
