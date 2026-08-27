import { createHash } from "node:crypto";
import { isRecord, type JsonRecord, type JsonValue } from "./extension-permissions";

const SUMMARY_LIMIT = 240;
const CONSENT_OPERATIONS = new Set([
  "agents.full_access_consent.issue",
  "agents.sessions.issue_full_access_consent",
  "agents.sessions.approve_full_access_consent",
  "agents.sessions.respond_full_access_consent",
]);

export function isFullAccessConsentOperation(operation: string): boolean {
  return CONSENT_OPERATIONS.has(operation);
}

export function assertMainRendererArkRequestAllowed(
  operation: string,
  fullAccessCreate = false,
): void {
  if (
    isFullAccessConsentOperation(operation) ||
    (operation === "agents.sessions.create" && fullAccessCreate)
  ) {
    throw new Error("[kepler-shell] full-access consent is host-only");
  }
}

export interface FullAccessConsentDetails {
  packageId: string;
  packageVersion: string;
  projectId: string;
  projectPath: string;
  prompt: string;
  promptSummary: string;
  promptHash: string;
  model: string;
}

export interface FullAccessConsentIssueParams {
  package_id: string;
  package_version: string;
  project_id: string;
  prompt: string;
  mode: "full-access";
  model?: string;
}

interface FullAccessConsentIssueResponse {
  request_id: string;
  package_id: string;
  package_version: string;
  project_id: string;
  project_path: string;
  mode: "full-access";
  model?: string | null;
  prompt_sha256: string;
  expires_at: string;
}

function isString(value: JsonValue | undefined): value is string {
  return typeof value === "string";
}

export function fullAccessSessionParams(
  params: JsonRecord | undefined,
): { projectId: string; prompt: string; model?: string } | null {
  if (params?.mode !== "full-access") return null;
  const projectId = params.project_id;
  const prompt = params.prompt;
  if (!isString(projectId) || !projectId || !isString(prompt) || !prompt) {
    throw new Error("FULL_ACCESS_INVALID: project_id and prompt are required");
  }
  const model = params.model;
  if (isString(model) && model) return { projectId, prompt, model };
  return { projectId, prompt };
}

export function withoutFullAccessConfirmation(params: JsonRecord | undefined): JsonRecord {
  const next = { ...(params ?? {}) };
  delete next.full_access_confirmed;
  delete next.consent_token;
  delete next.consent_nonce;
  return next;
}

export function buildFullAccessConsentDetails(input: {
  packageId: string;
  packageVersion: string;
  projectId: string;
  projectPath: string;
  prompt: string;
  model?: string;
}): FullAccessConsentDetails {
  const normalizedPrompt = input.prompt.replace(/\s+/g, " ").trim();
  return {
    ...input,
    model: input.model ?? "Default model",
    promptSummary:
      normalizedPrompt.length > SUMMARY_LIMIT
        ? `${normalizedPrompt.slice(0, SUMMARY_LIMIT - 1)}…`
        : normalizedPrompt,
    promptHash: createHash("sha256").update(input.prompt, "utf8").digest("hex"),
  };
}

export function fullAccessConsentIssue(
  value: JsonValue,
  expected: {
    packageId: string;
    packageVersion: string;
    projectId: string;
    prompt: string;
    model?: string;
  },
): FullAccessConsentIssueResponse | null {
  if (!isRecord(value)) return null;
  const expectedHash = createHash("sha256").update(expected.prompt, "utf8").digest("hex");
  if (
    !isString(value.request_id) ||
    !value.request_id ||
    !isString(value.project_path) ||
    !value.project_path ||
    !isString(value.expires_at) ||
    !Number.isFinite(Date.parse(value.expires_at)) ||
    value.package_id !== expected.packageId ||
    value.package_version !== expected.packageVersion ||
    value.project_id !== expected.projectId ||
    value.mode !== "full-access" ||
    (value.model ?? undefined) !== expected.model ||
    value.prompt_sha256 !== expectedHash
  ) {
    return null;
  }
  return {
    request_id: value.request_id,
    package_id: expected.packageId,
    package_version: expected.packageVersion,
    project_id: expected.projectId,
    project_path: value.project_path,
    mode: "full-access",
    model: expected.model,
    prompt_sha256: expectedHash,
    expires_at: value.expires_at,
  };
}

export function consentNonce(value: JsonValue): string | null {
  if (!isRecord(value)) return null;
  return isString(value.consent_nonce) && value.consent_nonce ? value.consent_nonce : null;
}
