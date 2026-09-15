export interface RunManifest {
  schemaVersion: 1;
  app: string;
  runId: string;
  runRoot: string;
  testRoot: string;
  dataDir: string;
  userDataDir: string;
  outputDir: string;
  ports: { shell: number };
  portLease: string | null;
  ownedPids: Array<{ pid: number; role: string; startTime: string; commandLine: string }>;
  createdAt: string;
}
export function readRunManifest(file: string, expectedRoot: string): RunManifest;
