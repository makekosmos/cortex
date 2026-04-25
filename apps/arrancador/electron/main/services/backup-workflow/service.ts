import { createBackupWorkflowInternals } from "./internals";
import { createBackupReadUseCases } from "./read-use-cases";
import type { BackupWorkflowContext, BackupWorkflowDeps } from "./types";
import { createBackupWriteUseCases } from "./write-use-cases";

export function createBackupWorkflow(deps: BackupWorkflowDeps) {
  const context: BackupWorkflowContext = {
    deps,
    io: createBackupWorkflowInternals(deps.internals),
  };

  return {
    ...createBackupReadUseCases(context),
    ...createBackupWriteUseCases(context),
  };
}

export type BackupWorkflow = ReturnType<typeof createBackupWorkflow>;
