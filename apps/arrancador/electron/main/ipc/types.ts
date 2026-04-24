import type { IpcMainInvokeEvent } from "electron";
import type { RuntimeState } from "../backend";

export type RuntimeHandler<TArgs extends unknown[], TResult> = (
  runtime: RuntimeState,
  ...args: TArgs
) => Promise<TResult> | TResult;

export type WithRuntime = <TArgs extends unknown[], TResult>(
  handler: RuntimeHandler<TArgs, TResult>,
) => (
  event: IpcMainInvokeEvent,
  ...args: TArgs
) => Promise<TResult>;
