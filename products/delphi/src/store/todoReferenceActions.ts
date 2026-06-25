import type { Ref } from "vue";
import { createArea } from "@/models/area";
import { createHeading } from "@/models/heading";
import { createProject, type CreateProjectParams } from "@/models/project";
import { createTag } from "@/models/tag";
import type { Area, Heading, Project, Tag, TodoItem } from "@/types/task";

export function createTodoReferenceActions(state: {
  todos: Ref<TodoItem[]>;
  projects: Ref<Project[]>;
  areas: Ref<Area[]>;
  tags: Ref<Tag[]>;
  headings: Ref<Heading[]>;
}) {
  const { todos, projects, areas, tags, headings } = state;

  function addProject(params: CreateProjectParams): Project {
    const project = createProject(params);
    projects.value = [project, ...projects.value];
    return project;
  }

  function updateProject(id: string, patch: Partial<Project>) {
    projects.value = projects.value.map((p) => (p.id === id ? { ...p, ...patch } : p));
  }

  function removeProject(id: string) {
    projects.value = projects.value.filter((p) => p.id !== id);
    todos.value = todos.value.map((t) => (t.projectId === id ? { ...t, projectId: null } : t));
  }

  function upsertProject(project: Project) {
    const exists = projects.value.some((p) => p.id === project.id);
    projects.value = exists
      ? projects.value.map((p) => (p.id === project.id ? project : p))
      : [project, ...projects.value];
  }

  function addArea(title: string): Area {
    const area = createArea(title);
    areas.value = [area, ...areas.value];
    return area;
  }

  function updateArea(id: string, patch: Partial<Area>) {
    areas.value = areas.value.map((a) => (a.id === id ? { ...a, ...patch } : a));
  }

  function removeArea(id: string) {
    areas.value = areas.value.filter((a) => a.id !== id);
    projects.value = projects.value.map((p) => (p.areaId === id ? { ...p, areaId: null } : p));
    todos.value = todos.value.map((t) => (t.areaId === id ? { ...t, areaId: null } : t));
  }

  function addTag(title: string, color?: string, shortcut?: string | null): Tag {
    const tag = createTag(title, color, shortcut);
    tags.value = [tag, ...tags.value];
    return tag;
  }

  function updateTag(id: string, patch: Partial<Tag>) {
    tags.value = tags.value.map((t) => (t.id === id ? { ...t, ...patch } : t));
  }

  function removeTag(id: string) {
    tags.value = tags.value.filter((t) => t.id !== id);
    todos.value = todos.value.map((t) =>
      t.tagIds.includes(id) ? { ...t, tagIds: t.tagIds.filter((tid) => tid !== id) } : t,
    );
  }

  function addHeading(title: string, projectId?: string | null): Heading {
    const heading = createHeading(title, projectId);
    headings.value = [heading, ...headings.value];
    return heading;
  }

  function updateHeading(id: string, patch: Partial<Heading>) {
    headings.value = headings.value.map((h) => (h.id === id ? { ...h, ...patch } : h));
  }

  function removeHeading(id: string) {
    headings.value = headings.value.filter((h) => h.id !== id);
    todos.value = todos.value.map((t) => (t.headingId === id ? { ...t, headingId: null } : t));
  }

  return {
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
  };
}
