import type { ArrancadorBridge } from "@/types/ipc";

export const arrancadorInvokeMock = vi.fn(
  (async (..._args: unknown[]) => undefined) as (
    ...args: unknown[]
  ) => Promise<unknown>,
);

export const arrancadorOnMock = vi.fn((..._args: unknown[]) => () => {});

export const resetArrancadorBridge = () => {
  arrancadorInvokeMock.mockReset();
  arrancadorInvokeMock.mockResolvedValue(undefined);
  arrancadorOnMock.mockReset();
  arrancadorOnMock.mockImplementation(() => () => {});

  window.arrancador = {
    invoke: arrancadorInvokeMock as unknown as ArrancadorBridge["invoke"],
    on: arrancadorOnMock as unknown as ArrancadorBridge["on"],
  };
};
