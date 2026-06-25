export function parentDir(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? "" : path.slice(0, index);
}

export function joinZipPath(base: string, href: string): string {
  if (!base) return normalizeZipPath(href);
  return normalizeZipPath(`${base}/${href}`);
}

export function titleFromHref(href: string): string {
  const fileName = href.split("/").at(-1) ?? href;
  return decodeURIComponent(fileName.replace(/\.[^.]+$/, "").replace(/[-_]+/g, " "));
}

function normalizeZipPath(path: string): string {
  const parts: string[] = [];
  for (const part of path.split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") parts.pop();
    else parts.push(part);
  }
  return parts.join("/");
}
