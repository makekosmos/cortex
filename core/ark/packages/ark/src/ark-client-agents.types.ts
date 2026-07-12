export type AgentsMode = "default" | "auto-review" | "full-access";
export type AgentsSessionStatus =
  | "starting"
  | "running"
  | "waiting_approval"
  | "completed"
  | "interrupted"
  | "failed"
  | "archived";

export interface AgentsProject {
  id: string;
  name: string;
  path: string;
  dirty: boolean;
  createdAt: string;
}

export interface AgentsSession {
  id: string;
  projectId: string;
  title: string;
  prompt: string;
  mode: AgentsMode;
  model: string | null;
  status: AgentsSessionStatus;
  branch: string;
  worktreePath: string;
  worktreeExists: boolean;
  baseCommit: string;
  codexThreadId: string | null;
  activeTurnId: string | null;
  createdAt: string;
  updatedAt: string;
  archivedAt: string | null;
}

export interface AgentsTimelineEvent {
  id: number;
  sessionId: string;
  kind: string;
  payload: unknown;
  createdAt: string;
  updatedAt: string;
  truncated: boolean;
}

export interface AgentsApproval {
  id: string;
  sessionId: string;
  requestId: unknown;
  method: string;
  params: unknown;
  status: "pending" | "resolved";
  response: unknown | null;
  createdAt: string;
  resolvedAt: string | null;
}

export type AgentsApprovalDecision = "accept" | "decline";
export type AgentsUserInputAnswers = Record<string, { answers: string[] }>;

export interface AgentsUserInputOption {
  label: string;
  description?: string;
  isOther?: boolean;
}

export interface AgentsUserInputQuestion {
  id: string;
  header?: string;
  question: string;
  options?: AgentsUserInputOption[];
}

export interface AgentsRequestUserInputParams {
  questions?: AgentsUserInputQuestion[];
}

export interface AgentsEvent {
  event: "agents_event";
  seq: number;
  kind:
    | "session_updated"
    | "timeline_appended"
    | "timeline_updated"
    | "approval_requested"
    | "approval_resolved"
    | "diff_updated";
  sessionId: string;
  payload: unknown;
}

export interface AgentsDiffFile {
  path: string;
  status: string;
  size: number;
  binary: boolean;
  contentOmitted: boolean;
}

export interface AgentsDiff {
  sessionId: string;
  baseCommit: string;
  files: AgentsDiffFile[];
  unifiedDiff: string;
  truncated: boolean;
}

export interface AgentsModel {
  id: string;
  displayName?: string;
  description?: string;
}

export interface AgentsEditor {
  id: "code" | "cursor" | "windsurf" | "explorer";
  label: string;
}

export interface AgentsSnapshot {
  seq: number;
  recentEvents: AgentsEvent[];
  projects: AgentsProject[];
  sessions: AgentsSession[];
  approvals: AgentsApproval[];
}

export interface ArkAgentsApi {
  projects: {
    list(): Promise<AgentsProject[]>;
    add(path: string): Promise<AgentsProject>;
    remove(projectId: string): Promise<void>;
  };
  sessions: {
    list(includeArchived?: boolean): Promise<AgentsSession[]>;
    get(sessionId: string): Promise<AgentsSession>;
    create(input: {
      projectId: string;
      prompt: string;
      mode: AgentsMode;
      model?: string;
      fullAccessConfirmed?: boolean;
    }): Promise<AgentsSession>;
    send(sessionId: string, text: string): Promise<void>;
    interrupt(sessionId: string): Promise<void>;
    archive(sessionId: string): Promise<void>;
    removeWorktree(sessionId: string): Promise<void>;
    timeline(
      sessionId: string,
      cursor?: number,
      limit?: number,
    ): Promise<{ events: AgentsTimelineEvent[]; nextCursor: number | null }>;
  };
  approvals: {
    respond(
      approvalId: string,
      decision: AgentsApprovalDecision,
      answers?: AgentsUserInputAnswers,
    ): Promise<void>;
  };
  diff: { get(sessionId: string): Promise<AgentsDiff> };
  models: { list(): Promise<{ data: AgentsModel[]; nextCursor?: string | null }> };
  editors: {
    list(): Promise<AgentsEditor[]>;
    open(sessionId: string, editorId: AgentsEditor["id"]): Promise<void>;
  };
  snapshot(afterSeq?: number): Promise<AgentsSnapshot>;
  onEvent(callback: (event: AgentsEvent) => void): () => void;
}
