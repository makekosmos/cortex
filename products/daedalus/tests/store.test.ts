import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import type { AgentsSession } from "@kosmos/ark/agents";

const mocks = vi.hoisted(() => ({
  timeline: vi.fn(),
  getSession: vi.fn(),
  removeWorktree: vi.fn(),
}));

vi.mock("@/services/agentsClient", () => ({
  agentsClient: {
    sessions: {
      timeline: mocks.timeline,
      get: mocks.getSession,
      removeWorktree: mocks.removeWorktree,
    },
    diff: { get: vi.fn() },
  },
}));
import { useAgentsStore } from "@/stores/agents";

const session = (overrides: Partial<AgentsSession> = {}): AgentsSession => ({
  id: "s",
  projectId: "p",
  title: "One",
  prompt: "Task",
  mode: "default",
  model: null,
  status: "running",
  branch: "codex/task-s",
  worktreePath: "C:\\worktree",
  worktreeExists: true,
  baseCommit: "abc",
  codexThreadId: "thread",
  activeTurnId: null,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
  archivedAt: null,
  ...overrides,
});

describe("agents store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.resetAllMocks();
  });

  it("ignores duplicate and stale seq", () => {
    const store = useAgentsStore();
    store.reduceEvent({
      event: "agents_event",
      seq: 2,
      kind: "session_updated",
      sessionId: "s",
      payload: session(),
    });
    store.reduceEvent({
      event: "agents_event",
      seq: 1,
      kind: "session_updated",
      sessionId: "s",
      payload: session({ title: "Stale" }),
    });
    expect(store.sessions[0]?.title).toBe("One");
  });

  it("moves archived sessions out of the active list", () => {
    const store = useAgentsStore();
    store.reduceEvent({
      event: "agents_event",
      seq: 1,
      kind: "session_updated",
      sessionId: "s",
      payload: session(),
    });
    store.selectedSessionId = "s";
    store.reduceEvent({
      event: "agents_event",
      seq: 2,
      kind: "session_updated",
      sessionId: "s",
      payload: session({ status: "archived", archivedAt: "2026-01-02T00:00:00Z" }),
    });
    expect(store.sessions).toHaveLength(0);
    expect(store.archivedSessions).toHaveLength(1);
    expect(store.selectedSession?.id).toBe("s");
  });

  it("loads older timeline pages without duplicates", async () => {
    mocks.timeline
      .mockResolvedValueOnce({
        events: [
          { id: 3, sessionId: "s", kind: "message", payload: {}, truncated: false },
          { id: 4, sessionId: "s", kind: "message", payload: {}, truncated: false },
        ],
        nextCursor: 3,
      })
      .mockResolvedValueOnce({
        events: [
          { id: 1, sessionId: "s", kind: "message", payload: {}, truncated: false },
          { id: 3, sessionId: "s", kind: "message", payload: {}, truncated: false },
        ],
        nextCursor: null,
      });
    const store = useAgentsStore();
    await store.selectSession("s");
    await store.loadOlder();
    expect(store.timeline.map((event) => event.id)).toEqual([1, 3, 4]);
    expect(store.timelineCursor).toBeNull();
  });

  it("refreshes worktree existence after confirmed removal", async () => {
    const store = useAgentsStore();
    store.reduceEvent({
      event: "agents_event",
      seq: 1,
      kind: "session_updated",
      sessionId: "s",
      payload: session({ status: "archived", archivedAt: "2026-01-02T00:00:00Z" }),
    });
    store.selectedSessionId = "s";
    mocks.getSession.mockResolvedValue(
      session({
        status: "archived",
        archivedAt: "2026-01-02T00:00:00Z",
        worktreeExists: false,
      }),
    );
    await store.removeWorktree();
    expect(mocks.removeWorktree).toHaveBeenCalledWith("s");
    expect(store.selectedSession?.worktreeExists).toBe(false);
  });

  it("keeps a dirty worktree visible when backend rejects removal", async () => {
    const store = useAgentsStore();
    store.reduceEvent({
      event: "agents_event",
      seq: 1,
      kind: "session_updated",
      sessionId: "s",
      payload: session({ status: "archived", archivedAt: "2026-01-02T00:00:00Z" }),
    });
    store.selectedSessionId = "s";
    mocks.removeWorktree.mockRejectedValue(new Error("Worktree содержит изменения"));
    await expect(store.removeWorktree()).resolves.toBe(false);
    expect(store.selectedSession?.worktreeExists).toBe(true);
    expect(store.error).toBe("Worktree содержит изменения");
  });
});
