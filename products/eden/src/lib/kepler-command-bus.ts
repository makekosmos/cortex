// In-memory command bridge for Eden running inside the Kepler shell.

export type EdenCommandChannel =
  | "eden:cmd:note:create"
  | "eden:cmd:note:search"
  | "eden:cmd:note:open-today";

const commandListeners = new Map<EdenCommandChannel, Set<(params: unknown) => void>>();
const pendingDispatches = new Map<EdenCommandChannel, unknown[]>();

export function dispatchEdenCommand(channel: EdenCommandChannel, params: unknown): void {
  const set = commandListeners.get(channel);
  if (!set || set.size === 0) {
    let queue = pendingDispatches.get(channel);
    if (!queue) {
      queue = [];
      pendingDispatches.set(channel, queue);
    }
    queue.push(params);
    return;
  }
  for (const handler of set) {
    try {
      handler(params);
    } catch (e) {
      console.warn(`[eden-extension] command handler ${channel} threw:`, e);
    }
  }
}

export function onCommand(
  channel: EdenCommandChannel,
  handler: (params: unknown) => void,
): () => void {
  let set = commandListeners.get(channel);
  if (!set) {
    set = new Set();
    commandListeners.set(channel, set);
  }
  set.add(handler);

  const pending = pendingDispatches.get(channel);
  if (pending && pending.length > 0) {
    pendingDispatches.delete(channel);
    queueMicrotask(() => {
      for (const params of pending) {
        try {
          handler(params);
        } catch (e) {
          console.warn(`[eden-extension] flushed handler ${channel} threw:`, e);
        }
      }
    });
  }

  return () => {
    set?.delete(handler);
  };
}
