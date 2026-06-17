import type { CommandRecord } from "@shared/ipc-types";

export function dedupeCommandsById(commands: readonly CommandRecord[]): CommandRecord[] {
  const seen = new Set<string>();
  const deduped: CommandRecord[] = [];
  for (const command of commands) {
    if (seen.has(command.id)) continue;
    seen.add(command.id);
    deduped.push(command);
  }
  return deduped;
}
