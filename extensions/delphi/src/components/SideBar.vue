<script setup lang="ts">
import { computed, shallowRef } from "vue";
import {
  Archive,
  ArrowLeft,
  Book,
  Globe,
  Inbox,
  Plus,
  Settings2,
  Star,
} from "lucide-vue-next";
import {
  Sidebar as KosmosSidebar,
  type SidebarConfig,
  type SidebarNavItem,
  type SidebarProjectGroup,
  type SidebarProjectItem,
} from "@kosmos/visuals";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { ProjectStatus } from "@/types/task";
import { setSidebarHidden } from "@/composables/useSidebarState";
import { useRoute, useRouter } from "vue-router";
import ProjectCreateDialog from "@/components/projects/ProjectCreateDialog.vue";
import type { ProjectCreatePayload } from "@/components/projects/ProjectCreateDialog.vue";

const STORAGE_KEY = "delphi-sidebar-config";

type SettingsTab = "general" | "spaces";

function loadConfig(): Partial<SidebarConfig> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw) as Partial<SidebarConfig>;
  } catch {
    // ignore
  }

  return {};
}

function saveConfig(config: SidebarConfig) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
  setSidebarHidden(config.hidden);
}

function colorTagClass(colorTag?: string | null): string {
  switch (colorTag) {
    case "red":
      return "bg-red-500";
    case "orange":
      return "bg-orange-500";
    case "yellow":
      return "bg-yellow-500";
    case "green":
      return "bg-green-500";
    case "blue":
      return "bg-blue-500";
    case "purple":
      return "bg-purple-500";
    case "pink":
      return "bg-pink-500";
    default:
      return "bg-(--muted-foreground)";
  }
}

const store = useTodoStore();
const { projects } = storeToRefs(store);
const route = useRoute();
const router = useRouter();
const initialConfig = loadConfig();
const isMac = navigator.platform.startsWith("Mac");
const projectCreateOpen = shallowRef(false);

const props = withDefaults(
  defineProps<{
    hidden?: boolean;
    showToggle?: boolean;
    reserveTopInset?: boolean;
  }>(),
  {
    hidden: undefined,
    showToggle: true,
    reserveTopInset: true,
  },
);

const isSettingsRoute = computed(() => route.path === "/settings");

const activeSettingsTab = computed<SettingsTab>(() => {
  const tabValue = Array.isArray(route.query.tab) ? route.query.tab[0] : route.query.tab;
  return tabValue === "spaces" ? "spaces" : "general";
});

const existingProjectTitles = computed(() => projects.value.map((project) => project.title));

const activeProjectItems = computed<SidebarProjectItem[]>(() =>
  projects.value
    .filter((project) => project.status === ProjectStatus.Active)
    .sort((left, right) => left.sortOrder - right.sortOrder)
    .map((project) => ({
      id: project.id,
      label: project.title,
      to: `/project/${project.id}`,
      active: route.path === `/project/${project.id}`,
      colorClass: colorTagClass(project.colorTag),
    })),
);

const projectGroups = computed<SidebarProjectGroup[]>(() => {
  if (isSettingsRoute.value) {
    return [];
  }

  return [
    {
      id: "projects",
      label: "Проекты",
      items: activeProjectItems.value,
      actionIcon: Plus,
      actionLabel: "Создать проект",
      actionTestId: "sidebar-create-project",
      onAction: () => {
        projectCreateOpen.value = true;
      },
    },
  ];
});

function setSettingsTab(tab: SettingsTab) {
  if (activeSettingsTab.value === tab) return;

  void router.replace({
    query: {
      ...route.query,
      tab,
    },
  });
}

function handleSettingsBack() {
  if (window.history.length > 1) {
    router.back();
    return;
  }

  void router.push("/");
}

function handleProjectCreate(payload: ProjectCreatePayload) {
  const project = store.addProject({
    title: payload.title,
    notes: payload.notes,
    colorTag: payload.colorTag,
    billable: payload.billable,
    price: payload.price,
  });

  projectCreateOpen.value = false;
  void router.push(`/project/${project.id}`);
}

const primaryItems = computed<SidebarNavItem[]>(() => {
  if (isSettingsRoute.value) {
    return [
      {
        id: "back",
        icon: ArrowLeft,
        label: "Назад",
        onClick: handleSettingsBack,
        testId: "settings-nav-back",
      },
      {
        id: "general",
        icon: Settings2,
        label: "Общие",
        active: activeSettingsTab.value === "general",
        onClick: () => setSettingsTab("general"),
        testId: "settings-nav-general",
      },
      {
        id: "spaces",
        icon: Globe,
        label: "Пространства",
        active: activeSettingsTab.value === "spaces",
        onClick: () => setSettingsTab("spaces"),
        testId: "settings-nav-spaces",
      },
    ];
  }

  return [
    { id: "inbox", icon: Inbox, to: "/", label: "Входящие" },
    { id: "today", icon: Star, to: "/today", label: "Сегодня" },
  ];
});

const footerItems = computed<SidebarNavItem[]>(() =>
  isSettingsRoute.value
    ? []
    : [
        { id: "logbook", icon: Book, to: "/logbook", label: "Журнал" },
        { id: "trash", icon: Archive, to: "/trash", label: "Корзина" },
      ],
);
</script>

<template>
  <KosmosSidebar
    :primary-items="primaryItems"
    :project-groups="projectGroups"
    :footer-items="footerItems"
    :is-mac="isMac"
    :default-width="200"
    :min-width="160"
    :max-width="320"
    toggle-shortcut="meta+b|ctrl+b"
    :initial-config="initialConfig"
    :hidden="props.hidden"
    :show-toggle="props.showToggle"
    :reserve-top-inset="props.reserveTopInset"
    @config-change="saveConfig"
  />

  <ProjectCreateDialog
    v-model:open="projectCreateOpen"
    :existing-titles="existingProjectTitles"
    @save="handleProjectCreate"
  />
</template>
