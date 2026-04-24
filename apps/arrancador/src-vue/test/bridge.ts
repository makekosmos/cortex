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
    commands: new Proxy(
      {},
      {
        get: (_target, property) => {
          if (typeof property !== "string") {
            return undefined;
          }
          return (payload?: unknown) => arrancadorInvokeMock(property, payload);
        },
      },
    ) as ArrancadorBridge["commands"],
    on: arrancadorOnMock as unknown as ArrancadorBridge["on"],
  };
};
