export type RaycastChild =
  | RaycastElement
  | string
  | number
  | boolean
  | null
  | undefined
  | RaycastChild[];

export interface RaycastElement<TType extends string = string, TProps = Record<string, unknown>> {
  readonly $$typeof: "kosmos.raycast.element";
  readonly type: TType;
  readonly props: TProps & { children?: RaycastChild[] };
}

export interface Icon {
  source: string;
  tintColor?: string;
}

export type ImageLike = string | Icon;

export interface LaunchProps<TArguments = Record<string, unknown>, TContext = unknown> {
  arguments?: TArguments;
  launchContext?: TContext;
  launchType: LaunchTypeValue;
  fallbackText?: string;
}

export const LaunchType = {
  UserInitiated: "userInitiated",
  Background: "background",
  LaunchCommand: "launchCommand",
} as const;

export type LaunchTypeValue = (typeof LaunchType)[keyof typeof LaunchType];

export interface LaunchCommandOptions {
  name: string;
  extensionName?: string;
  type?: LaunchTypeValue;
  context?: unknown;
  arguments?: Record<string, unknown>;
  fallbackText?: string;
}

export interface AlertOptions {
  title: string;
  message?: string;
  primaryAction?: { title: string; style?: "default" | "destructive" };
  dismissAction?: { title: string };
}

export interface PreferenceValues {
  [key: string]: unknown;
}
