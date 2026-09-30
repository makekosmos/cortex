const SUPPORTED_STATUSES = new Set(["A", "M", "D", "R", "C"]);

export function parseNameStatus(output) {
  const tokens = output.split("\0").filter(Boolean);
  const files = [];
  for (let index = 0; index < tokens.length;) {
    const status = tokens[index++];
    if (!status || !SUPPORTED_STATUSES.has(status[0])) return null;
    if (/^[RC]/.test(status)) {
      const oldPath = tokens[index++];
      const newPath = tokens[index++];
      if (!oldPath || !newPath) return null;
      files.push(
        { path: oldPath.replaceAll("\\", "/"), status: status[0] },
        { path: newPath.replaceAll("\\", "/"), status: status[0] },
      );
    } else {
      const path = tokens[index++];
      if (!path) return null;
      files.push({ path: path.replaceAll("\\", "/"), status: status[0] });
    }
  }
  return files;
}

// Reads one side of a diff: "index" (staged blob), "worktree" (file on disk),
// or any commit-ish. Returns null when the revision cannot be read.
export function readRevision(git, readFile, revision, path) {
  if (revision === "worktree") {
    try {
      return readFile(path);
    } catch {
      return null;
    }
  }
  const spec = revision === "index" ? `:${path}` : `${revision}:${path}`;
  const result = git(["show", spec]);
  return result.error || result.status !== 0 ? null : result.stdout;
}
