import { computed, shallowRef } from "vue";
import { defineStore } from "pinia";
import type {
  AgentsApproval,
  AgentsDiff,
  AgentsEditor,
  AgentsEvent,
  AgentsMode,
  AgentsModel,
  AgentsProject,
  AgentsSession,
  AgentsTimelineEvent,
  AgentsUserInputAnswers,
} from "@kosmos/ark/agents";
import { agentsClient } from "@/services/agentsClient";

export const useAgentsStore = defineStore("agents", () => {
  const projects = shallowRef<AgentsProject[]>([]);
  const sessions = shallowRef<AgentsSession[]>([]);
  const archivedSessions = shallowRef<AgentsSession[]>([]);
  const approvals = shallowRef<AgentsApproval[]>([]);
  const models = shallowRef<AgentsModel[]>([]);
  const editors = shallowRef<AgentsEditor[]>([]);
  const timeline = shallowRef<AgentsTimelineEvent[]>([]);
  const timelineCursor = shallowRef<number | null>(null);
  const timelineLoadingOlder = shallowRef(false);
  const diff = shallowRef<AgentsDiff | null>(null);
  const selectedSessionId = shallowRef<string | null>(null);
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);
  const lastSeq = shallowRef(0);

  const selectedSession = computed(
    () =>
      [...sessions.value, ...archivedSessions.value].find(
        (session) => session.id === selectedSessionId.value,
      ) ?? null,
  );
  const pendingApproval = computed(
    () =>
      approvals.value.find((approval) => approval.sessionId === selectedSessionId.value) ?? null,
  );

  function reduceEvent(event: AgentsEvent): void {
    if (event.seq <= lastSeq.value) return;
    lastSeq.value = event.seq;
    if (event.kind === "session_updated") {
      const session = event.payload as AgentsSession;
      reduceSession(session);
    } else if (event.kind === "timeline_appended" && event.sessionId === selectedSessionId.value) {
      const item = event.payload as AgentsTimelineEvent;
      if (typeof item?.id === "number") timeline.value = [...timeline.value, item];
    } else if (event.kind === "timeline_updated" && event.sessionId === selectedSessionId.value) {
      const item = event.payload as AgentsTimelineEvent;
      timeline.value = timeline.value.map((current) => (current.id === item.id ? item : current));
    } else if (event.kind === "approval_requested") {
      const approval = event.payload as AgentsApproval;
      approvals.value = [approval, ...approvals.value.filter((item) => item.id !== approval.id)];
    } else if (event.kind === "approval_resolved") {
      const id = (event.payload as { approvalId?: string })?.approvalId;
      approvals.value = approvals.value.filter((item) => item.id !== id);
    }
    if (event.kind === "diff_updated" && event.sessionId === selectedSessionId.value)
      void loadDiff();
  }

  async function initialize(): Promise<() => void> {
    loading.value = true;
    let hydrating = true;
    const queued: AgentsEvent[] = [];
    const unsubscribe = agentsClient.onEvent((event) => {
      if (hydrating) queued.push(event);
      else reduceEvent(event);
    });
    try {
      const snapshot = await agentsClient.snapshot(lastSeq.value);
      projects.value = snapshot.projects;
      sessions.value = snapshot.sessions;
      approvals.value = snapshot.approvals;
      const replay = [...(snapshot.recentEvents ?? []), ...queued].sort((a, b) => a.seq - b.seq);
      lastSeq.value = Math.min(lastSeq.value, snapshot.seq);
      for (const event of replay) reduceEvent(event);
      lastSeq.value = Math.max(lastSeq.value, snapshot.seq);
      hydrating = false;
      selectedSessionId.value ??= sessions.value[0]?.id ?? null;
      const [modelResponse, availableEditors, allSessions] = await Promise.all([
        agentsClient.models.list().catch(() => ({ data: [] })),
        agentsClient.editors.list().catch(() => [{ id: "explorer" as const, label: "Проводник" }]),
        agentsClient.sessions.list(true).catch(() => snapshot.sessions),
      ]);
      models.value = modelResponse.data;
      editors.value = availableEditors;
      archivedSessions.value = allSessions.filter(
        (session) => session.status === "archived" || session.archivedAt !== null,
      );
      if (selectedSessionId.value) await selectSession(selectedSessionId.value);
      return unsubscribe;
    } catch (cause) {
      hydrating = false;
      unsubscribe();
      error.value = cause instanceof Error ? cause.message : String(cause);
      return () => undefined;
    } finally {
      loading.value = false;
    }
  }

  async function addProject(): Promise<void> {
    const path = await window.kepler?.dialogs.pickDirectory();
    if (!path) return;
    const project = await agentsClient.projects.add(path);
    projects.value = [...projects.value.filter((item) => item.id !== project.id), project];
  }

  function newSession(): void {
    selectedSessionId.value = null;
    timeline.value = [];
    timelineCursor.value = null;
    diff.value = null;
  }

  async function createSession(input: {
    projectId: string;
    prompt: string;
    mode: AgentsMode;
    model?: string;
    fullAccessConfirmed?: boolean;
  }): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      const session = await agentsClient.sessions.create(input);
      sessions.value = [session, ...sessions.value];
      await selectSession(session.id);
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loading.value = false;
    }
  }

  async function selectSession(id: string): Promise<void> {
    selectedSessionId.value = id;
    timeline.value = [];
    timelineCursor.value = null;
    const page = await agentsClient.sessions.timeline(id, undefined, 250);
    if (selectedSessionId.value !== id) return;
    const merged = new Map(page.events.map((event) => [event.id, event]));
    for (const event of timeline.value) merged.set(event.id, event);
    timeline.value = [...merged.values()].sort((a, b) => a.id - b.id);
    timelineCursor.value = page.nextCursor;
    await loadDiff(id);
  }
  async function loadOlder(): Promise<void> {
    const sessionId = selectedSessionId.value;
    const cursor = timelineCursor.value;
    if (!sessionId || cursor === null || timelineLoadingOlder.value) return;
    timelineLoadingOlder.value = true;
    try {
      const page = await agentsClient.sessions.timeline(sessionId, cursor, 250);
      if (selectedSessionId.value !== sessionId) return;
      const merged = new Map(timeline.value.map((event) => [event.id, event]));
      for (const event of page.events) merged.set(event.id, event);
      timeline.value = [...merged.values()].sort((a, b) => a.id - b.id);
      timelineCursor.value = page.nextCursor;
    } finally {
      timelineLoadingOlder.value = false;
    }
  }
  async function send(text: string): Promise<void> {
    if (selectedSessionId.value) await agentsClient.sessions.send(selectedSessionId.value, text);
  }
  async function interrupt(): Promise<void> {
    if (selectedSessionId.value) await agentsClient.sessions.interrupt(selectedSessionId.value);
  }
  async function archive(): Promise<void> {
    if (!selectedSessionId.value) return;
    const archivedId = selectedSessionId.value;
    await agentsClient.sessions.archive(archivedId);
    reduceSession(await agentsClient.sessions.get(archivedId));
  }
  function reduceSession(session: AgentsSession): void {
    const archived = session.status === "archived" || session.archivedAt !== null;
    if (archived) {
      sessions.value = sessions.value.filter((item) => item.id !== session.id);
      archivedSessions.value = [
        session,
        ...archivedSessions.value.filter((item) => item.id !== session.id),
      ];
      return;
    }
    sessions.value = [session, ...sessions.value.filter((item) => item.id !== session.id)];
    archivedSessions.value = archivedSessions.value.filter((item) => item.id !== session.id);
  }
  async function removeWorktree(): Promise<boolean> {
    const sessionId = selectedSessionId.value;
    if (!sessionId) return false;
    error.value = null;
    try {
      await agentsClient.sessions.removeWorktree(sessionId);
      reduceSession(await agentsClient.sessions.get(sessionId));
      diff.value = null;
      return true;
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
      return false;
    }
  }
  async function openEditor(editorId: AgentsEditor["id"]): Promise<void> {
    if (selectedSessionId.value) await agentsClient.editors.open(selectedSessionId.value, editorId);
  }
  async function loadDiff(sessionId = selectedSessionId.value): Promise<void> {
    if (
      !sessionId ||
      selectedSessionId.value !== sessionId ||
      !selectedSession.value?.worktreeExists
    ) {
      diff.value = null;
      return;
    }
    const loaded = await agentsClient.diff.get(sessionId);
    if (selectedSessionId.value === sessionId) diff.value = loaded;
  }
  async function respond(
    approvalId: string,
    decision: "accept" | "decline",
    answers?: AgentsUserInputAnswers,
  ): Promise<void> {
    await agentsClient.approvals.respond(approvalId, decision, answers);
  }

  return {
    projects,
    sessions,
    archivedSessions,
    approvals,
    models,
    editors,
    timeline,
    timelineCursor,
    timelineLoadingOlder,
    diff,
    loading,
    error,
    selectedSessionId,
    selectedSession,
    pendingApproval,
    initialize,
    reduceEvent,
    newSession,
    addProject,
    createSession,
    selectSession,
    loadOlder,
    send,
    interrupt,
    archive,
    removeWorktree,
    openEditor,
    loadDiff,
    respond,
  };
});
