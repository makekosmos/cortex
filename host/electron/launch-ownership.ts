export type OwnedLaunch = Readonly<{
  id: string;
  owner: object;
  webContentsId: number;
  generation: number;
  launchId: string;
}>;

export type ClaimResult = Readonly<{
  current: OwnedLaunch;
  replaced?: OwnedLaunch;
}>;

/**
 * Electron-free authority for locally-owned Engine launch leases.
 * Local ownership is always removed before a best-effort remote revoke, so a
 * failed revoke never leaves a stale window able to affect a replacement.
 */
export class LaunchOwnership {
  private readonly claims = new Map<string, OwnedLaunch>();
  private generation = 0;

  claim(id: string, owner: object, webContentsId: number, launchId: string): ClaimResult {
    const replaced = this.claims.get(id);
    const current: OwnedLaunch = {
      id,
      owner,
      webContentsId,
      launchId,
      generation: ++this.generation,
    };
    this.claims.set(id, current);
    return { current, ...(replaced ? { replaced } : {}) };
  }

  current(id: string): OwnedLaunch | undefined {
    return this.claims.get(id);
  }

  size(): number {
    return this.claims.size;
  }

  /** Removes the exact current claim and returns its launch ID for revocation. */
  take(claim: OwnedLaunch): string | undefined {
    if (this.claims.get(claim.id) !== claim) return undefined;
    this.claims.delete(claim.id);
    return claim.launchId;
  }

  async release(
    claim: OwnedLaunch,
    revoke: (launchId: string) => Promise<unknown>,
  ): Promise<boolean> {
    const launchId = this.take(claim);
    if (!launchId) return false;
    await Promise.allSettled([revoke(launchId)]);
    return true;
  }

  async drain(revoke: (launchId: string) => Promise<unknown>): Promise<void> {
    const launches = [...this.claims.values()];
    this.claims.clear();
    await Promise.allSettled(launches.map((launch) => revoke(launch.launchId)));
  }
}
