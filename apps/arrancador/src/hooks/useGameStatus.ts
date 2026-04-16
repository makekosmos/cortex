import { useEffect, useRef, useState } from "react";
import { gamesApi } from "@/lib/api";

export function useGameStatus(gameId?: string, exePath?: string) {
  const [isInstalled, setIsInstalled] = useState(false);
  const [checkingInstalled, setCheckingInstalled] = useState(false);
  const [runningCount, setRunningCount] = useState(0);
  const [checkingRunning, setCheckingRunning] = useState(false);
  const installRequestIdRef = useRef(0);
  const runningRequestIdRef = useRef(0);

  useEffect(() => {
    if (!gameId) {
      setIsInstalled(false);
      setCheckingInstalled(false);
      return;
    }

    const requestId = ++installRequestIdRef.current;
    let mounted = true;
    setIsInstalled(false);
    setCheckingInstalled(true);
    gamesApi
      .isInstalled(gameId)
      .then((installed) => {
        if (mounted && requestId === installRequestIdRef.current) {
          setIsInstalled(installed);
        }
      })
      .catch((e) => {
        console.error("Failed to check install status:", e);
        if (mounted && requestId === installRequestIdRef.current) {
          setIsInstalled(true);
        }
      })
      .finally(() => {
        if (mounted && requestId === installRequestIdRef.current) {
          setCheckingInstalled(false);
        }
      });

    return () => {
      mounted = false;
    };
  }, [gameId, exePath]);

  useEffect(() => {
    if (!gameId) {
      setRunningCount(0);
      setCheckingRunning(false);
      return;
    }

    let mounted = true;
    let timeoutId: ReturnType<typeof setTimeout> | undefined;
    let cancelled = false;

    setRunningCount(0);

    const updateRunning = async () => {
      const requestId = ++runningRequestIdRef.current;
      setCheckingRunning(true);
      try {
        const count = await gamesApi.getRunningInstances(gameId);
        if (mounted && !cancelled && requestId === runningRequestIdRef.current) {
          setRunningCount(count);
        }
      } catch (e) {
        if (mounted && !cancelled) {
          console.error("Failed to check running instances:", e);
        }
      } finally {
        if (mounted && !cancelled && requestId === runningRequestIdRef.current) {
          setCheckingRunning(false);
        }
        if (mounted && !cancelled) {
          timeoutId = setTimeout(updateRunning, 5000);
        }
      }
    };

    updateRunning();
    return () => {
      mounted = false;
      cancelled = true;
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
    };
  }, [gameId]);

  return {
    isInstalled,
    checkingInstalled,
    runningCount,
    checkingRunning,
    setRunningCount,
  };
}
