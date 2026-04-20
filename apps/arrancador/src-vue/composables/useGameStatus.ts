import { shallowRef, watch } from "vue";
import { gamesApi } from "../../src/lib/api";

export function useGameStatus(gameId?: () => string | undefined, exePath?: () => string | undefined) {
  const isInstalled = shallowRef(false);
  const checkingInstalled = shallowRef(false);
  const runningCount = shallowRef(0);
  const checkingRunning = shallowRef(false);

  let installRequestId = 0;
  let runningRequestId = 0;
  let runningTimer: ReturnType<typeof setTimeout> | undefined;

  watch(
    [() => gameId?.(), () => exePath?.()],
    ([nextGameId]) => {
      if (!nextGameId) {
        isInstalled.value = false;
        checkingInstalled.value = false;
        return;
      }

      const requestId = ++installRequestId;
      isInstalled.value = false;
      checkingInstalled.value = true;

      void gamesApi
        .isInstalled(nextGameId)
        .then((installed) => {
          if (requestId === installRequestId) {
            isInstalled.value = installed;
          }
        })
        .catch((cause) => {
          console.error("Failed to check install status:", cause);
          if (requestId === installRequestId) {
            isInstalled.value = true;
          }
        })
        .finally(() => {
          if (requestId === installRequestId) {
            checkingInstalled.value = false;
          }
        });
    },
    { immediate: true },
  );

  watch(
    () => gameId?.(),
    (nextGameId, _previousGameId, onCleanup) => {
      if (!nextGameId) {
        runningCount.value = 0;
        checkingRunning.value = false;
        return;
      }

      let cancelled = false;
      runningCount.value = 0;

      const updateRunning = async () => {
        const requestId = ++runningRequestId;
        checkingRunning.value = true;

        try {
          const count = await gamesApi.getRunningInstances(nextGameId);
          if (!cancelled && requestId === runningRequestId) {
            runningCount.value = count;
          }
        } catch (cause) {
          if (!cancelled) {
            console.error("Failed to check running instances:", cause);
          }
        } finally {
          if (!cancelled && requestId === runningRequestId) {
            checkingRunning.value = false;
          }

          if (!cancelled) {
            runningTimer = setTimeout(updateRunning, 5000);
          }
        }
      };

      void updateRunning();

      onCleanup(() => {
        cancelled = true;
        if (runningTimer) {
          clearTimeout(runningTimer);
          runningTimer = undefined;
        }
      });
    },
    { immediate: true },
  );

  return {
    isInstalled,
    checkingInstalled,
    runningCount,
    checkingRunning,
    setRunningCount: (count: number) => {
      runningCount.value = count;
    },
  };
}
