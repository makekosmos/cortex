import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { expect, type Page } from "@playwright/test";
import { waitForPidGone } from "./host-runtime";

type Session = {
  id?: string;
  status?: string;
  activeTurnId?: string | null;
};

type RpcResult = { ok: boolean; data?: unknown };

export const fakeAppServerEnvironment = (script: string, pidFile: string): NodeJS.ProcessEnv => ({
  KOSMOS_TEST_MODE: "1",
  DAEDALUS_FAKE_APP_SERVER_EXE: process.execPath,
  DAEDALUS_FAKE_APP_SERVER_SCRIPT: script,
  DAEDALUS_FAKE_APP_SERVER_PID_FILE: pidFile,
});

export const createGitProject = (root: string): string => {
  const project = path.join(root, "daedalus-project");
  fs.mkdirSync(project, { recursive: true });
  const git = (args: string[]) =>
    execFileSync("git", args, { cwd: project, stdio: "ignore", windowsHide: true });
  git(["init"]);
  git(["config", "user.email", "daedalus@test.invalid"]);
  git(["config", "user.name", "Daedalus Test"]);
  fs.writeFileSync(path.join(project, "README.md"), "fixture\n", "utf8");
  git(["add", "README.md"]);
  git(["commit", "-m", "base"]);
  return project;
};

export const createAndSend = async (page: Page, projectId: string, prompt: string, text: string) =>
  page.evaluate(
    async ({ projectId, prompt, text }) => {
      const created = await window.kosmosApp.ark.request("agents.sessions.create", {
        project_id: projectId,
        prompt,
        mode: "default",
      });
      // SAFETY: the id is validated as a String before it is used below.
      const session = created.data as { id?: unknown } | undefined;
      if (!created.ok || session?.id?.constructor !== String)
        throw new Error(`session create failed: ${JSON.stringify(created)}`);
      const sent = await window.kosmosApp.ark.request("agents.sessions.send", {
        session_id: session.id,
        text,
      });
      return { created, sent, sessionId: session.id };
    },
    { projectId, prompt, text },
  );

const getSession = async (page: Page, sessionId: string): Promise<Session | null> =>
  page.evaluate(async (sessionId) => {
    // SAFETY: only the stable ok/data envelope is read before filtering validated session fields.
    const listed = (await window.kosmosApp.ark.request("agents.sessions.list", {
      include_archived: true,
    })) as RpcResult;
    if (!listed.ok) return null;
    // SAFETY: malformed entries cannot match the validated string session id.
    return (listed.data as Session[]).find((session) => session.id === sessionId) ?? null;
  }, sessionId);

export const waitForSession = async (
  page: Page,
  sessionId: string,
  expected: Partial<Session>,
): Promise<void> => {
  await expect.poll(() => getSession(page, sessionId)).toMatchObject(expected);
};

const getApproval = async (page: Page, sessionId: string) =>
  page.evaluate(async (sessionId) => {
    // SAFETY: only the stable ok/data envelope is read before the approval id is validated.
    const snapshot = (await window.kosmosApp.ark.request("agents.snapshot", {
      after_seq: 0,
    })) as RpcResult;
    if (!snapshot.ok) return null;
    // SAFETY: optional approval fields are checked before use by the caller.
    const approvals =
      (
        snapshot.data as {
          approvals?: Array<{ id?: string; sessionId?: string }>;
        }
      ).approvals ?? [];
    return approvals.find((approval) => approval.sessionId === sessionId) ?? null;
  }, sessionId);

export const waitForApproval = async (page: Page, sessionId: string): Promise<string> => {
  await expect
    .poll(() => getApproval(page, sessionId))
    .toMatchObject({
      id: expect.any(String),
    });
  const approval = await getApproval(page, sessionId);
  if (!approval?.id) throw new Error(`approval missing for session ${sessionId}`);
  return approval.id;
};

export const acceptApproval = (page: Page, approvalId: string) =>
  page.evaluate(
    (approvalId) =>
      window.kosmosApp.ark.request("agents.approvals.respond", {
        approval_id: approvalId,
        decision: "accept",
      }),
    approvalId,
  );

type AppServerTree = { root: number; child: number; mode: string };

const appServerTrees = (pidFile: string): AppServerTree[] =>
  fs.existsSync(pidFile)
    ? fs
        .readFileSync(pidFile, "utf8")
        .trim()
        .split(/\r?\n/)
        .filter(Boolean)
        .map((line) => {
          const parsed = JSON.parse(line);
          // SAFETY: the test owns this JSONL file and writes this exact shape.
          return parsed as AppServerTree;
        })
    : [];

const waitForNewAppServerTree = async (pidFile: string, mode: string): Promise<AppServerTree> => {
  let tree: AppServerTree | undefined;
  await expect
    .poll(() => {
      tree = appServerTrees(pidFile).find((candidate) => candidate.mode === mode);
      return tree;
    })
    .toEqual({ root: expect.any(Number), child: expect.any(Number), mode });
  // SAFETY: expect.poll above returns only after tree matches the complete shape.
  return tree!;
};

export const waitForProcessCleanup = async (pids: number[], label: string): Promise<void> => {
  for (const pid of pids) await waitForPidGone(pid, label);
};

export const runSessionLifecycle = async (
  page: Page,
  projectId: string,
  pidFile: string,
  pids: Set<number>,
): Promise<void> => {
  const happySession = await createAndSend(
    page,
    projectId,
    "Daedalus packaged happy session",
    "complete the happy packaged session",
  );
  expect(happySession.created).toMatchObject({ ok: true, data: { status: "running" } });
  expect(happySession.sent).toEqual({ ok: true, data: true });
  const happyApproval = await waitForApproval(page, happySession.sessionId);
  expect(await acceptApproval(page, happyApproval)).toEqual({ ok: true, data: true });
  await waitForSession(page, happySession.sessionId, {
    status: "completed",
    activeTurnId: null,
  });

  const interruptSession = await createAndSend(
    page,
    projectId,
    "Daedalus interrupt-unresponsive active process",
    "interrupt active process",
  );
  expect(interruptSession.created).toMatchObject({ ok: true });
  expect(interruptSession.sent).toEqual({ ok: true, data: true });
  await waitForSession(page, interruptSession.sessionId, {
    activeTurnId: expect.any(String),
  });
  const interruptTree = await waitForNewAppServerTree(pidFile, "interrupt");
  const interruptDescendants = [interruptTree.root, interruptTree.child];
  pids.add(...interruptDescendants);
  expect(
    await page.evaluate(
      (sessionId) =>
        window.kosmosApp.ark.request("agents.sessions.interrupt", { session_id: sessionId }),
      interruptSession.sessionId,
    ),
  ).toEqual({ ok: true, data: true });
  await waitForSession(page, interruptSession.sessionId, {
    status: "interrupted",
    activeTurnId: null,
  });
  await waitForProcessCleanup(interruptDescendants, "interrupted app-server");

  const archiveSession = await createAndSend(
    page,
    projectId,
    "Daedalus archive active process",
    "archive while waiting approval",
  );
  expect(archiveSession.created).toMatchObject({ ok: true });
  expect(archiveSession.sent).toEqual({ ok: true, data: true });
  await waitForApproval(page, archiveSession.sessionId);
  await waitForSession(page, archiveSession.sessionId, {
    activeTurnId: expect.any(String),
  });
  const archiveTree = await waitForNewAppServerTree(pidFile, "archive");
  const archiveDescendants = [archiveTree.root, archiveTree.child];
  pids.add(...archiveDescendants);
  expect(
    await page.evaluate(
      (sessionId) =>
        window.kosmosApp.ark.request("agents.sessions.archive", { session_id: sessionId }),
      archiveSession.sessionId,
    ),
  ).toEqual({ ok: true, data: true });
  await waitForSession(page, archiveSession.sessionId, {
    status: "archived",
    activeTurnId: null,
  });
  await waitForProcessCleanup(archiveDescendants, "archived app-server");
};
