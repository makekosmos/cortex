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
