import { BrowserWindow, dialog, type MessageBoxOptions, type WebFrameMain } from "electron";
import { isRecord, type JsonRecord, type JsonValue } from "./extension-permissions";
import {
  buildFullAccessConsentDetails,
  consentNonce,
  fullAccessConsentIssue,
  fullAccessSessionParams,
  type FullAccessConsentDetails,
  type FullAccessConsentIssueParams,
} from "./full-access-consent";

type Request = (request: JsonRecord) => Promise<JsonValue>;

interface Context {
  id: string;
  version?: string;
}

async function showWarning(
  details: FullAccessConsentDetails,
  parent: BrowserWindow | null,
): Promise<boolean> {
  if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") return false;
  const options: MessageBoxOptions = {
    type: "warning",
    buttons: ["Разрешить full-access", "Отмена"],
    defaultId: 1,
    cancelId: 1,
    title: "Разрешить Cortex full-access?",
    message: `${details.packageId} хочет запустить агента с полным доступом`,
    detail: [
      `Пакет: ${details.packageId} v${details.packageVersion}`,
      `Проект: ${details.projectPath}`,
      `Модель: ${details.model}`,
      `Промпт: ${details.promptSummary}`,
      `SHA-256 промпта: ${details.promptHash}`,
      "",
      "Full SYSTEM scope: full-access отключает sandbox и обычные approval-запросы.",
      "Агент сможет читать и изменять любую файловую систему, запускать любые процессы и обращаться к сети.",
      "Разрешение действует только для этой операции.",
    ].join("\n"),
  };
  const result = await (
    parent && !parent.isDestroyed()
      ? dialog.showMessageBox(parent, options)
      : dialog.showMessageBox(options)
  ).catch(() => ({ response: 1 }));
  return result.response === 0;
}

export async function createFullAccessSession(
  context: Context,
  params: JsonRecord,
  request: Request,
  parent: BrowserWindow | null,
  requesterFrame: WebFrameMain,
): Promise<JsonValue> {
  const input = fullAccessSessionParams(params);
  if (!input) throw new Error("FULL_ACCESS_INVALID: full-access parameters are required");
  const packageVersion = context.version;
  if (!packageVersion) throw new Error("FULL_ACCESS_INVALID: package version unavailable");
  const issueParams: FullAccessConsentIssueParams = {
    package_id: context.id,
    package_version: packageVersion,
    project_id: input.projectId,
    prompt: input.prompt,
    mode: "full-access",
  };
  if (input.model) issueParams.model = input.model;
  const issued = await request({
    operation: "agents.sessions.issue_full_access_consent",
    ...issueParams,
  });
  const consentRequest = fullAccessConsentIssue(issued, {
    packageId: context.id,
    packageVersion,
    projectId: input.projectId,
    prompt: input.prompt,
    model: input.model,
  });
  if (!consentRequest) {
    throw new Error("FULL_ACCESS_INVALID: Runtime returned mismatched consent metadata");
  }
  const details = buildFullAccessConsentDetails({
    packageId: context.id,
    packageVersion,
    projectId: input.projectId,
    projectPath: consentRequest.project_path,
    prompt: input.prompt,
    model: input.model,
  });
  const approved =
    !requesterFrame.isDestroyed() &&
    (await showWarning(details, parent)) &&
    !requesterFrame.isDestroyed();
  if (!approved) {
    await request({
      operation: "agents.sessions.approve_full_access_consent",
      request_id: consentRequest.request_id,
      approved: false,
    });
    throw new Error("FULL_ACCESS_DECLINED: operation cancelled");
  }
  try {
    const approval = await request({
      operation: "agents.sessions.approve_full_access_consent",
      request_id: consentRequest.request_id,
      approved: true,
    });
    if (!isRecord(approval) || approval.approved !== true) {
      throw new Error("FULL_ACCESS_INVALID: Runtime did not approve this operation");
    }
    const nonce = consentNonce(approval);
    if (!nonce) throw new Error("FULL_ACCESS_INVALID: Runtime returned no consent nonce");
    if (requesterFrame.isDestroyed()) {
      throw new Error("FULL_ACCESS_DECLINED: requesting renderer was reloaded");
    }
    const createParams: JsonRecord = {
      operation: "agents.sessions.create",
      project_id: input.projectId,
      prompt: input.prompt,
      mode: "full-access",
      consent_nonce: nonce,
      package_id: context.id,
      package_version: packageVersion,
    };
    if (input.model) createParams.model = input.model;
    return await request(createParams);
  } catch (error) {
    await request({
      operation: "agents.sessions.approve_full_access_consent",
      request_id: consentRequest.request_id,
      approved: false,
    }).catch(() => {});
    throw error;
  }
}
