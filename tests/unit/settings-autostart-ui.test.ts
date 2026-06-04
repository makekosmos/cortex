import { describe, expect, test } from "bun:test";
import {
  resolveAutostartApplyFailure,
  resolveAutostartApplySuccess,
} from "../../shell/src/views/settings/autostart-ui";

describe("settings autostart UI state", () => {
  test("does not surface a false apply error after set succeeds", () => {
    // Regression: 2026-06-04. Windows/Electron autorun readback can lag right
    // after set; a successful write should update the toggle to the requested
    // state without showing "Не удалось применить настройку".
    expect(resolveAutostartApplySuccess(true)).toEqual({
      state: "applied",
      checked: true,
      error: "",
    });
  });

  test("keeps a registry write failure visible", () => {
    expect(resolveAutostartApplyFailure(false)).toEqual({
      state: "write_failed",
      checked: false,
      error: "Ошибка записи в реестр",
    });
  });
});
