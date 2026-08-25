import type { ArkClient } from "@kosmos/ark";

import type { PomodoroPhase } from "./focus-session-types";

let backendEventsUnsubscribe: (() => void) | null = null;
let pendingWorkCompletion = false;

export function setupFocusSessionBackendSync(opts: {
  arkClient: ArkClient;
  closeTimeEntry: (completed: boolean) => Promise<void>;
  applyFocusState: (active: boolean) => Promise<void>;
  enqueueSideEffect: (work: () => Promise<void>) => Promise<void>;
  onWorkEnded: () => void;
  broadcastUpdated: () => void;
}): void {
  teardownFocusSessionBackendSync();
  backendEventsUnsubscribe = opts.arkClient.onArkEvent((event) => {
    if (event.event === "pomodoro_finished" && event.finished === "work") {
      pendingWorkCompletion = true;
      return;
    }
    if (event.event !== "pomodoro_phase_changed") return;
// SAFETY: The surrounding boundary establishes this documented contract.
    const from = (event as { from?: PomodoroPhase }).from ?? "idle";
// SAFETY: The surrounding boundary establishes this documented contract.
    const to = (event as { to?: PomodoroPhase }).to ?? "idle";
    if (from === "work") {
      const completed = pendingWorkCompletion;
      pendingWorkCompletion = false;
      void opts.enqueueSideEffect(async () => {
        await opts.closeTimeEntry(completed);
        if (to !== "work") {
          opts.onWorkEnded();
          await opts.applyFocusState(false);
        }
        opts.broadcastUpdated();
      });
    }
  });
}

export function teardownFocusSessionBackendSync(): void {
  if (backendEventsUnsubscribe) {
    try {
      backendEventsUnsubscribe();
    } catch (e) {
      console.error("[focus-session] teardown unsubscribe failed:", e);
    }
    backendEventsUnsubscribe = null;
  }
  pendingWorkCompletion = false;
}
