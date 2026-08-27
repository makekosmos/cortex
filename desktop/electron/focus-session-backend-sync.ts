import type { ArkClient } from "@kosmos/ark";

import type { PomodoroPhase } from "./focus-session-types";

let backendEventsUnsubscribe: (() => void) | null = null;
let pendingWorkCompletion = false;

export function setupFocusSessionBackendSync(opts: {
  arkClient: ArkClient;
  closeTimeEntry: (completed: boolean) => Promise<void>;
  applyFocusState: (active: boolean) => Promise<void>;
  enqueueSideEffect: (work: () => Promise<void>) => Promise<void>;
  onWorkEnded: (nextPhase: PomodoroPhase) => void;
  onWorkStarted: () => void;
  onFocusEnforcementError: (active: boolean, error: Error) => void;
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
        let enforcementError: Error | null = null;
        let closeError: Error | null = null;
        if (to !== "work") {
          try {
            await opts.applyFocusState(false);
          } catch (error) {
            enforcementError = error instanceof Error ? error : new Error(String(error));
          }
        }
        try {
          await opts.closeTimeEntry(completed);
        } catch (error) {
          closeError = error instanceof Error ? error : new Error(String(error));
        }
        if (!enforcementError && to !== "work") opts.onWorkEnded(to);
        if (enforcementError) {
          opts.onFocusEnforcementError(false, enforcementError);
        }
        opts.broadcastUpdated();
        if (enforcementError && closeError) {
          throw new Error(
            `${enforcementError.message}; time entry close failed: ${closeError.message}`,
          );
        }
        if (enforcementError) throw enforcementError;
        if (closeError) throw closeError;
      });
      return;
    }
    if (to === "work") {
      void opts.enqueueSideEffect(async () => {
        try {
          await opts.applyFocusState(true);
          opts.onWorkStarted();
          opts.broadcastUpdated();
        } catch (error) {
          opts.onFocusEnforcementError(
            true,
            error instanceof Error ? error : new Error(String(error)),
          );
          throw error;
        }
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
