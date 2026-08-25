export type Timer = {
  setTimeout(callback: () => void, ms: number): ReturnType<typeof setTimeout>;
  clearTimeout(handle: ReturnType<typeof setTimeout>): void;
};

export class HostLifecycle {
  private windows = 0;
  private hasOpened = false;
  private pending: ReturnType<typeof setTimeout> | null = null;
  constructor(
    private readonly exit: () => void,
    private readonly timer: Timer = { setTimeout, clearTimeout },
    private warmTimeout: 0 | 300 = 300,
  ) {}

  setWarmTimeout(timeout: 0 | 300): void {
    this.warmTimeout = timeout;
    if (this.hasOpened && this.windows === 0 && timeout === 0) this.exitNow();
  }
  opened(): void {
    this.windows += 1;
    this.hasOpened = true;
    if (this.pending !== null) {
      this.timer.clearTimeout(this.pending);
      this.pending = null;
    }
  }
  closed(): void {
    this.windows = Math.max(0, this.windows - 1);
    if (this.windows !== 0) return;
    if (this.warmTimeout === 0) this.exitNow();
    else
      this.pending = this.timer.setTimeout(() => {
        this.pending = null;
        if (this.windows === 0) this.exit();
      }, 300_000);
  }
  private exitNow(): void {
    if (this.pending !== null) this.timer.clearTimeout(this.pending);
    this.pending = null;
    this.exit();
  }
}
