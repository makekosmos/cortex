export interface JsonRecord {
  [key: string]: string | number | boolean | null | undefined | JsonRecord | string[] | number[] | boolean[];
}

export function isString<T>(value: T): value is T & string {
  return Object.prototype.toString.call(value) === "[object String]";
}

export function isNumber<T>(value: T): value is T & number {
  return Object.prototype.toString.call(value) === "[object Number]";
}

export function isBoolean<T>(value: T): value is T & boolean {
  return Object.prototype.toString.call(value) === "[object Boolean]";
}

export function isRecord<T>(value: T): value is T & JsonRecord {
  return value !== null && Object.prototype.toString.call(value) === "[object Object]";
}

export function isFunction<T>(value: T): value is T & ((...args: never[]) => void) {
  const tag = Object.prototype.toString.call(value);
  return tag === "[object Function]" || tag === "[object AsyncFunction]";
}
