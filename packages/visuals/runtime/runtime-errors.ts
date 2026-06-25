import type { App } from "vue";

export interface ConsoleOnlyRuntimeErrorsOptions {
  label?: string;
  preventDefault?: boolean;
}

type WindowWithRuntimeErrorHook = Window & {
  __kosmosConsoleOnlyRuntimeErrors?: boolean;
};

function describeReason(reason: unknown): unknown {
  if (reason instanceof Error) return reason;
  return reason ?? "(no error detail)";
}

export function installConsoleOnlyRuntimeErrors(
  app: App,
  options: ConsoleOnlyRuntimeErrorsOptions = {},
): void {
  const label = options.label ?? "kosmos";
  const preventDefault = options.preventDefault ?? true;

  app.config.errorHandler = (err, _instance, info) => {
    console.error(`[${label}] Vue runtime error${info ? ` (${info})` : ""}:`, describeReason(err));
  };

  const globalWindow = window as WindowWithRuntimeErrorHook;
  if (globalWindow.__kosmosConsoleOnlyRuntimeErrors) return;
  globalWindow.__kosmosConsoleOnlyRuntimeErrors = true;

  window.addEventListener("error", (event) => {
    console.error(
      `[${label}] Unhandled runtime error:`,
      describeReason(event.error ?? event.message),
    );
    if (preventDefault) event.preventDefault();
  });

  window.addEventListener("unhandledrejection", (event) => {
    console.error(`[${label}] Unhandled promise rejection:`, describeReason(event.reason));
    if (preventDefault) event.preventDefault();
  });
}
