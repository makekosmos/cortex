type FileLikeWithPath = File & { path?: string };
type DragTransferEvent = { dataTransfer?: DataTransfer | null };

type PickOptions = {
  directory?: boolean;
  multiple?: boolean;
  title?: string;
};

const INPUT_ACCEPT = ".exe,.lnk";

const isBrowser = typeof window !== "undefined";
const electronBridge = () =>
  typeof window === "undefined" ? undefined : window.arrancador;

const filePathFromFile = (file: File): string => {
  const fileLike = file as FileLikeWithPath;
  if (typeof fileLike.path === "string" && fileLike.path.trim()) {
    return fileLike.path;
  }
  if (file.webkitRelativePath) {
    return file.webkitRelativePath;
  }
  return file.name;
};

const normalizePath = (value: string) => value.split("\\").join("/");

const toNativeSeparators = (value: string) =>
  value.includes("\\") ? value.split("/").join("\\") : value;

const dirname = (value: string) => {
  const normalized = normalizePath(value);
  const trimmed = normalized.replace(/\/+$/, "");
  const lastSlash = trimmed.lastIndexOf("/");
  if (lastSlash <= 0) {
    return trimmed;
  }
  return trimmed.slice(0, lastSlash);
};

const commonDirectory = (values: string[]) => {
  if (values.length === 0) return "";
  if (values.length === 1) return dirname(values[0]);

  const splitValues = values.map((value) => normalizePath(value).split("/"));
  const shortest = Math.min(...splitValues.map((parts) => parts.length));
  const shared: string[] = [];

  for (let index = 0; index < shortest; index += 1) {
    const current = splitValues[0][index];
    if (!current) break;
    if (splitValues.some((parts) => parts[index] !== current)) break;
    shared.push(current);
  }

  if (shared.length === 0) {
    return dirname(values[0]);
  }

  return shared.join("/");
};

const fileUrlFromPath = (value: string) => {
  const normalized = value.split("\\").join("/");
  if (/^[a-zA-Z]:\//.test(normalized)) {
    return `file:///${encodeURI(normalized)}`;
  }
  if (normalized.startsWith("//")) {
    return `file:${encodeURI(normalized)}`;
  }
  if (normalized.startsWith("/")) {
    return `file://${encodeURI(normalized)}`;
  }
  return normalized;
};

const pickFromInput = (options: PickOptions) =>
  new Promise<FileList | null>((resolve) => {
    if (!isBrowser || typeof document === "undefined") {
      resolve(null);
      return;
    }

    const input = document.createElement("input");
    input.type = "file";
    input.style.position = "fixed";
    input.style.left = "-9999px";
    input.style.top = "-9999px";
    input.accept = INPUT_ACCEPT;
    input.multiple = Boolean(options.multiple);
    if (options.directory) {
      input.setAttribute("webkitdirectory", "");
      input.setAttribute("directory", "");
    }

    let resolved = false;

    const cleanup = () => {
      input.value = "";
      input.remove();
    };

    const finish = (files: FileList | null) => {
      if (resolved) return;
      resolved = true;
      cleanup();
      resolve(files && files.length > 0 ? files : null);
    };

    input.addEventListener("change", () => {
      finish(input.files);
    });

    window.addEventListener(
      "focus",
      () => {
        window.setTimeout(() => {
          finish(input.files);
        }, 150);
      },
      { once: true },
    );

    document.body.appendChild(input);
    input.click();
  });

export async function pickFilePath(options: PickOptions = {}) {
  const bridge = electronBridge();
  if (bridge) {
    const result = await bridge.commands.dialog_open({
      directory: false,
      multiple: false,
      title: options.title,
    });
    return typeof result === "string" ? result : null;
  }

  const files = await pickFromInput({ ...options, multiple: false });
  if (!files || files.length === 0) return null;
  return filePathFromFile(files[0]);
}

export async function pickDirectoryPath(options: PickOptions = {}) {
  const bridge = electronBridge();
  if (bridge) {
    const result = await bridge.commands.dialog_open({
      directory: true,
      multiple: false,
      title: options.title,
    });
    return typeof result === "string" ? result : null;
  }

  const files = await pickFromInput({ ...options, directory: true });
  if (!files || files.length === 0) return null;
  const paths = Array.from(files)
    .map(filePathFromFile)
    .filter((value) => value.trim().length > 0);
  if (paths.length === 0) return null;
  return toNativeSeparators(commonDirectory(paths));
}

export async function pickDirectoryFiles(options: PickOptions = {}) {
  const files = await pickFromInput({ ...options, directory: true });
  if (!files || files.length === 0) return [];
  return Array.from(files)
    .map(filePathFromFile)
    .filter((value) => value.trim().length > 0);
}

export async function openPath(target: string) {
  if (!isBrowser || typeof window === "undefined" || !target.trim()) {
    return;
  }

  const bridge = electronBridge();
  if (bridge) {
    const value = target.trim();
    if (/^https?:\/\//i.test(value)) {
      await bridge.commands.shell_open_external({ url: value });
    } else {
      await bridge.commands.shell_open_path({ path: value });
    }
    return;
  }

  const resolved = /^https?:\/\//i.test(target.trim()) ? target.trim() : fileUrlFromPath(target.trim());

  const opened = window.open(resolved, "_blank", "noopener,noreferrer");
  if (!opened && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(target.trim());
  }
}

export function subscribeAppEvent<T>(
  eventName: string,
  handler: (payload: T) => void,
) {
  if (!isBrowser || typeof window === "undefined") {
    return () => {};
  }

  const bridge = electronBridge();
  if (bridge) {
    return bridge.on(eventName as never, handler as never);
  }

  const listener = (event: Event) => {
    handler((event as CustomEvent<T>).detail);
  };

  window.addEventListener(eventName, listener);
  return () => {
    window.removeEventListener(eventName, listener);
  };
}

export async function getAutoStartState() {
  const bridge = electronBridge();
  if (bridge) {
    return await bridge.commands.get_autostart_state();
  }

  return false;
}

export async function setAutoStartState(enabled: boolean) {
  const bridge = electronBridge();
  if (bridge) {
    await bridge.commands.set_autostart_state({ enabled });
    return;
  }

  void enabled;
}

export function extractDroppedPaths(event: DragTransferEvent) {
  const files = event.dataTransfer?.files;
  if (!files || files.length === 0) return [];

  return Array.from(files)
    .map(filePathFromFile)
    .filter((value) => value.trim().length > 0);
}
