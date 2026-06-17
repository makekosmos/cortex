export type AutostartApplyResult =
  | { state: "applied"; checked: boolean; error: "" }
  | { state: "write_failed"; checked: boolean; error: string };

export function resolveAutostartApplySuccess(desired: boolean): AutostartApplyResult {
  return { state: "applied", checked: desired, error: "" };
}

export function resolveAutostartApplyFailure(current: boolean): AutostartApplyResult {
  return { state: "write_failed", checked: current, error: "Ошибка записи в реестр" };
}
