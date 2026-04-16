import type {
  DashboardLoadOptions,
  DashboardPlatform,
  DashboardSnapshot,
} from "@shared/analytics";

declare global {
  interface Window {
    dashboardApi: {
      loadSnapshot(options?: DashboardLoadOptions): Promise<DashboardSnapshot>;
      chooseDatabase(): Promise<DashboardSnapshot>;
      resetDatabase(): Promise<DashboardSnapshot>;
      getDefaultDbPath(): Promise<string>;
      getPlatform(): Promise<DashboardPlatform>;
      minimize(): void;
      maximize(): void;
      close(): void;
    };
  }
}

export {};
