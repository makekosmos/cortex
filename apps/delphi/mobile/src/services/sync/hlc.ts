/**
 * Hybrid Logical Clock for sync conflict resolution.
 *
 * Format: "<ISO8601>:<counter:06d>:<device_id>"
 */
export class HLC {
  constructor(
    public readonly wallTime: string,
    public readonly counter: number,
    public readonly deviceId: string,
  ) {}

  static now(deviceId: string): HLC {
    return new HLC(new Date().toISOString(), 0, deviceId);
  }

  tick(): HLC {
    const now = new Date().toISOString();
    if (this.wallTime < now) {
      return new HLC(now, 0, this.deviceId);
    }
    return new HLC(this.wallTime, this.counter + 1, this.deviceId);
  }

  merge(remote: HLC): HLC {
    const now = new Date().toISOString();
    const maxTime = [now, this.wallTime, remote.wallTime].sort().pop()!;

    let counter: number;
    if (maxTime === this.wallTime && maxTime === remote.wallTime) {
      counter = Math.max(this.counter, remote.counter) + 1;
    } else if (maxTime === this.wallTime) {
      counter = this.counter + 1;
    } else if (maxTime === remote.wallTime) {
      counter = remote.counter + 1;
    } else {
      counter = 0;
    }

    return new HLC(maxTime, counter, this.deviceId);
  }

  toString(): string {
    const paddedCounter = String(this.counter).padStart(6, "0");
    return `${this.wallTime}:${paddedCounter}:${this.deviceId}`;
  }

  static fromString(s: string): HLC {
    const firstColon = s.indexOf(":", s.indexOf("Z"));
    const secondColon = s.indexOf(":", firstColon + 1);
    const wallTime = s.slice(0, firstColon);
    const counter = parseInt(s.slice(firstColon + 1, secondColon), 10);
    const deviceId = s.slice(secondColon + 1);
    return new HLC(wallTime, counter, deviceId);
  }

  compareTo(other: HLC): number {
    if (this.wallTime < other.wallTime) return -1;
    if (this.wallTime > other.wallTime) return 1;
    if (this.counter < other.counter) return -1;
    if (this.counter > other.counter) return 1;
    if (this.deviceId < other.deviceId) return -1;
    if (this.deviceId > other.deviceId) return 1;
    return 0;
  }

  static compare(a: HLC, b: HLC): number {
    return a.compareTo(b);
  }
}
