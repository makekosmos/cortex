import type { AgentsEvent, ArkAgentsApi } from "./ark-client-agents.types.js";
export type * from "./ark-client-agents.types.js";

export type AgentsRequest = <T>(operation: string, params?: Record<string, unknown>) => Promise<T>;

export function createArkAgentsApi(
  request: AgentsRequest,
  subscribe: (callback: (event: AgentsEvent) => void) => () => void,
): ArkAgentsApi {
  return {
    projects: {
      list: () => request("agents.projects.list"),
      add: (path) => request("agents.projects.add", { path }),
      remove: async (projectId) => {
        await request("agents.projects.remove", { project_id: projectId });
      },
    },
    sessions: {
      list: (includeArchived = false) =>
        request("agents.sessions.list", { include_archived: includeArchived }),
      get: (sessionId) => request("agents.sessions.get", { session_id: sessionId }),
      create: (input) =>
        request("agents.sessions.create", {
          project_id: input.projectId,
          prompt: input.prompt,
          mode: input.mode,
          model: input.model,
          full_access_confirmed: input.fullAccessConfirmed,
        }),
      send: async (sessionId, text) => {
        await request("agents.sessions.send", { session_id: sessionId, text });
      },
      interrupt: async (sessionId) => {
        await request("agents.sessions.interrupt", { session_id: sessionId });
      },
      archive: async (sessionId) => {
        await request("agents.sessions.archive", { session_id: sessionId });
      },
      removeWorktree: async (sessionId) => {
        await request("agents.sessions.remove_worktree", { session_id: sessionId });
      },
      timeline: (sessionId, cursor, limit = 100) =>
        request("agents.sessions.timeline", { session_id: sessionId, cursor, limit }),
    },
    approvals: {
      respond: async (approvalId, decision, answers) => {
        await request("agents.approvals.respond", { approval_id: approvalId, decision, answers });
      },
    },
    diff: { get: (sessionId) => request("agents.diff.get", { session_id: sessionId }) },
    models: { list: () => request("agents.models.list") },
    editors: {
      list: () => request("agents.editors.list"),
      open: async (sessionId, editorId) => {
        await request("agents.editors.open", { session_id: sessionId, editor_id: editorId });
      },
    },
    snapshot: (afterSeq = 0) => request("agents.snapshot", { after_seq: afterSeq }),
    onEvent: subscribe,
  };
}
