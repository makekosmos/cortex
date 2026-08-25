export type IpcJsonPrimitive = string | number | boolean | null | undefined;
export type IpcJsonFunction = (...args: never[]) => void;
export type IpcJsonValue =
  | IpcJsonPrimitive
  | Date
  | IpcJsonFunction
  | IpcJsonObject
  | IpcJsonValue[];

export interface IpcJsonObject {
  [key: string]: IpcJsonValue | undefined;
}
