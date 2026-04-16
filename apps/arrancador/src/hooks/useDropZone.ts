import { useCallback, useEffect, useRef, useState } from "react";
import type { DragEvent as ReactDragEvent } from "react";
import { extractDroppedPaths, getTauriWindow } from "@/lib/browser";

type DropZoneOptions = {
  onDropPaths: (paths: string[]) => void | Promise<void>;
};

type ExternalDropPayload = {
  type?: "enter" | "over" | "leave" | "drop";
  paths?: string[];
};

const hasSupportedDragFiles = (event: DragEvent | ReactDragEvent) => {
  const types = Array.from(event.dataTransfer?.types ?? []);
  return types.includes("Files");
};

const isSupportedPath = (value: string) => {
  const lower = value.toLowerCase();
  return lower.endsWith(".exe") || lower.endsWith(".lnk");
};

const hasSupportedPaths = (paths: string[] | undefined) =>
  Array.isArray(paths) && paths.some(isSupportedPath);

export function useDropZone({ onDropPaths }: DropZoneOptions) {
  const [dropActive, setDropActive] = useState(false);
  const depthRef = useRef(0);

  const clearDropState = useCallback(() => {
    depthRef.current = 0;
    setDropActive(false);
  }, []);

  const onDragEnter = useCallback((event: ReactDragEvent) => {
    if (!hasSupportedDragFiles(event)) return;
    event.preventDefault();
    depthRef.current += 1;
    setDropActive(true);
  }, []);

  const onDragOver = useCallback((event: ReactDragEvent) => {
    if (!hasSupportedDragFiles(event)) return;
    event.preventDefault();
    setDropActive(true);
  }, []);

  const onDragLeave = useCallback((event: ReactDragEvent) => {
    if (!hasSupportedDragFiles(event)) return;
    event.preventDefault();
    depthRef.current = Math.max(0, depthRef.current - 1);
    if (depthRef.current === 0) {
      setDropActive(false);
    }
  }, []);

  const onDrop = useCallback(
    async (event: ReactDragEvent) => {
      if (!hasSupportedDragFiles(event)) return;
      event.preventDefault();
      clearDropState();
      const paths = extractDroppedPaths(event);
      if (paths.length > 0) {
        await onDropPaths(paths);
      }
    },
    [clearDropState, onDropPaths],
  );

  useEffect(() => {
    let disposed = false;
    let release: (() => void) | null = null;

    void getTauriWindow().then(async (appWindow) => {
      if (!appWindow || disposed) {
        return;
      }

      const unlisten = await appWindow.onDragDropEvent(async (event) => {
        const payload = event.payload as ExternalDropPayload | undefined;
        const eventType = payload?.type;
        const paths = payload?.paths ?? [];

        switch (eventType) {
          case "enter":
          case "over":
            setDropActive(hasSupportedPaths(paths));
            break;
          case "leave":
            setDropActive(false);
            break;
          case "drop":
            clearDropState();
            if (paths.length > 0) {
              await onDropPaths(paths);
            }
            break;
          default:
            break;
        }
      });

      if (disposed) {
        unlisten();
        return;
      }

      release = unlisten;
    });

    return () => {
      disposed = true;
      release?.();
    };
  }, [clearDropState, onDropPaths]);

  return {
    dropActive,
    dropHandlers: {
      onDragEnter,
      onDragOver,
      onDragLeave,
      onDrop,
    },
  };
}
