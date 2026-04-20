import { onMounted, onUnmounted, shallowRef } from "vue";
import { extractDroppedPaths, getTauriWindow } from "../../src/lib/browser";

type DropZoneOptions = {
  onDropPaths: (paths: string[]) => void | Promise<void>;
};

type ExternalDropPayload = {
  type?: "enter" | "over" | "leave" | "drop";
  paths?: string[];
};

function hasSupportedDragFiles(event: DragEvent) {
  const types = Array.from(event.dataTransfer?.types ?? []);
  return types.includes("Files");
}

function isSupportedPath(value: string) {
  const lower = value.toLowerCase();
  return lower.endsWith(".exe") || lower.endsWith(".lnk");
}

function hasSupportedPaths(paths: string[] | undefined) {
  return Array.isArray(paths) && paths.some(isSupportedPath);
}

export function useDropZone({ onDropPaths }: DropZoneOptions) {
  const dropActive = shallowRef(false);
  let depth = 0;
  let release: (() => void) | null = null;
  let disposed = false;

  function clearDropState() {
    depth = 0;
    dropActive.value = false;
  }

  function onDragEnter(event: DragEvent) {
    if (!hasSupportedDragFiles(event)) return;
    event.preventDefault();
    depth += 1;
    dropActive.value = true;
  }

  function onDragOver(event: DragEvent) {
    if (!hasSupportedDragFiles(event)) return;
    event.preventDefault();
    dropActive.value = true;
  }

  function onDragLeave(event: DragEvent) {
    if (!hasSupportedDragFiles(event)) return;
    event.preventDefault();
    depth = Math.max(0, depth - 1);
    if (depth === 0) {
      dropActive.value = false;
    }
  }

  async function onDrop(event: DragEvent) {
    if (!hasSupportedDragFiles(event)) return;
    event.preventDefault();
    clearDropState();

    const paths = extractDroppedPaths(event);
    if (paths.length > 0) {
      await onDropPaths(paths);
    }
  }

  onMounted(() => {
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
            dropActive.value = hasSupportedPaths(paths);
            break;
          case "leave":
            dropActive.value = false;
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
  });

  onUnmounted(() => {
    disposed = true;
    release?.();
  });

  return {
    dropActive,
    dropHandlers: {
      onDragenter: onDragEnter,
      onDragover: onDragOver,
      onDragleave: onDragLeave,
      onDrop,
    },
  };
}
