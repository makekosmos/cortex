<script setup lang="ts">
import { computed, h, shallowRef, type Component } from "vue";
import {
  PhArchive,
  PhArrowLeft,
  PhBookOpen,
  PhFolder,
  PhGear,
  PhGlobe,
  PhStar,
  PhTrash,
  PhTray,
} from "@phosphor-icons/vue";
import {
  Button,
  SettingsSidebar,
  SettingsSidebarButton,
  type SidebarNavItem,
} from "@kosmos/visuals";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { ProjectStatus } from "@/types/task";
import { useRoute, useRouter } from "vue-router";
import ProjectCreateDialog from "@/components/projects/ProjectCreateDialog.vue";
import type { ProjectCreatePayload } from "@/components/projects/ProjectCreateDialog.vue";

type SettingsTab = "general" | "spaces";

const store = useTodoStore();
const { projects } = storeToRefs(store);
const route = useRoute();
const router = useRouter();
const projectCreateOpen = shallowRef(false);

function phosphorSidebarIcon(icon: Component, active = false): Component {
  return {
    inheritAttrs: false,
    setup(_, { attrs }) {
      return () => h(icon, { ...attrs, size: 16, weight: active ? "fill" : "duotone" });
    },
  };
}

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

const activeProjectItems = computed(() =>
  projects.value
    .filter((project) => project.status === ProjectStatus.Active)
    .sort((left, right) => left.sortOrder - right.sortOrder)
    .map((project) => ({
      id: project.id,
      label: project.title,
      to: `/project/${project.id}`,
      active: route.path === `/project/${project.id}`,
    })),
);

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

function navigateTo(path: string) {
  if (route.path === path) return;
  void router.push(path);
}

const primaryItems = computed<SidebarNavItem[]>(() => {
  if (isSettingsRoute.value) {
    return [
      {
        id: "back",
        icon: phosphorSidebarIcon(PhArrowLeft),
        label: "Назад",
        onClick: handleSettingsBack,
        testId: "settings-nav-back",
      },
      {
        id: "general",
        icon: phosphorSidebarIcon(PhGear, activeSettingsTab.value === "general"),
        label: "Общие",
        active: activeSettingsTab.value === "general",
        onClick: () => setSettingsTab("general"),
        testId: "settings-nav-general",
      },
      {
        id: "spaces",
        icon: phosphorSidebarIcon(PhGlobe, activeSettingsTab.value === "spaces"),
        label: "Пространства",
        active: activeSettingsTab.value === "spaces",
        onClick: () => setSettingsTab("spaces"),
        testId: "settings-nav-spaces",
      },
    ];
  }

  return [
    {
      id: "inbox",
      icon: phosphorSidebarIcon(PhTray, route.path === "/"),
      label: "Входящие",
      active: route.path === "/",
      onClick: () => navigateTo("/"),
    },
    {
      id: "today",
      icon: phosphorSidebarIcon(PhStar, route.path === "/today"),
      label: "Сегодня",
      active: route.path === "/today",
      onClick: () => navigateTo("/today"),
    },
    {
      id: "someday",
      icon: phosphorSidebarIcon(PhArchive, route.path === "/someday"),
      label: "Когда-нибудь",
      active: route.path === "/someday",
      onClick: () => navigateTo("/someday"),
    },
  ];
});

const footerItems = computed<SidebarNavItem[]>(() =>
  isSettingsRoute.value
    ? []
    : [
        {
          id: "logbook",
          icon: phosphorSidebarIcon(PhBookOpen, route.path === "/logbook"),
          label: "Журнал",
          active: route.path === "/logbook",
          onClick: () => navigateTo("/logbook"),
        },
        {
          id: "trash",
          icon: phosphorSidebarIcon(PhTrash, route.path === "/trash"),
          label: "Корзина",
          active: route.path === "/trash",
          onClick: () => navigateTo("/trash"),
        },
      ],
);
</script>

<template>
  <SettingsSidebar
    v-if="!props.hidden"
    tone="strong"
    :title="isSettingsRoute ? 'Настройки' : 'Delphi'"
  >
    <div class="flex min-h-0 flex-1 flex-col gap-6 px-2 pb-2">
      <div class="flex flex-col gap-1">
        <SettingsSidebarButton
          v-for="item in primaryItems"
          :key="item.id"
          :icon="item.icon"
          :label="item.label ?? ''"
          :active="item.active"
          icon-variant="plain"
          @click="item.onClick?.()"
        />
      </div>

      <div v-if="!isSettingsRoute" class="flex min-h-0 flex-1 flex-col gap-2">
        <div class="flex items-center justify-between px-1">
          <span class="text-[11px] font-medium text-[var(--muted-foreground)]">Проекты</span>
          <Button type="button" size="sm" variant="ghost" @click="projectCreateOpen = true">
            Новый
          </Button>
        </div>

        <div class="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto kosmos-scroll">
          <SettingsSidebarButton
            v-for="project in activeProjectItems"
            :key="project.id"
            :icon="phosphorSidebarIcon(PhFolder, project.active)"
            :label="project.label"
            :active="project.active"
            icon-variant="plain"
            @click="navigateTo(project.to)"
          />
        </div>
      </div>

      <div v-if="footerItems.length > 0" class="mt-auto flex flex-col gap-1">
        <SettingsSidebarButton
          v-for="item in footerItems"
          :key="item.id"
          :icon="item.icon"
          :label="item.label ?? ''"
          :active="item.active"
          icon-variant="plain"
          @click="item.onClick?.()"
        />
      </div>
    </div>
  </SettingsSidebar>

  <ProjectCreateDialog
    v-if="!isSettingsRoute"
    v-model:open="projectCreateOpen"
    :existing-titles="existingProjectTitles"
    @save="handleProjectCreate"
  />
</template>
