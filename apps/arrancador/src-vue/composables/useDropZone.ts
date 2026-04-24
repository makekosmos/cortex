import { shallowRef } from "vue";
import { extractDroppedPaths } from "../../src/lib/browser";

type DropZoneOptions = {
  onDropPaths: (paths: string[]) => void | Promise<void>;
};

function hasSupportedDragFiles(event: DragEvent) {
  const types = Array.from(event.dataTransfer?.types ?? []);
  return types.includes("Files");
}

export function useDropZone({ onDropPaths }: DropZoneOptions) {
  const dropActive = shallowRef(false);
  let depth = 0;

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
