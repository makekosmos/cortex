import { readdir } from "node:fs/promises";
import path from "node:path";

function blank(source, start, end) {
  let result = "";
  for (let index = start; index < end; index++)
    result += source[index] === "\n" || source[index] === "\r" ? source[index] : " ";
  return result;
}

function rawStringEnd(source, start) {
  const prefix = source.slice(start).match(/^(?:br|cr|r)(#{0,255})"/);
  if (!prefix) return null;
  const closing = `"${prefix[1]}`;
  const closingStart = source.indexOf(closing, start + prefix[0].length);
  if (closingStart < 0) throw new Error(`unterminated raw string at byte ${start}`);
  return closingStart + closing.length;
}

function quotedStringEnd(source, quoteStart) {
  for (let index = quoteStart + 1; index < source.length; index++) {
    if (source[index] === "\\") index++;
    else if (source[index] === '"') return index + 1;
  }
  throw new Error(`unterminated string at byte ${quoteStart}`);
}

function charLiteralEnd(source, quoteStart) {
  let index = quoteStart + 1;
  if (source[index] === "\\") {
    index++;
    if (source[index] === "u" && source[index + 1] === "{") {
      const close = source.indexOf("}", index + 2);
      if (close < 0) return null;
      index = close + 1;
    } else index++;
  } else if (index < source.length) index += source.codePointAt(index) > 0xffff ? 2 : 1;
  return source[index] === "'" ? index + 1 : null;
}

// Preserve offsets while hiding Rust comments and literals from source scans.
export function maskRustTrivia(source) {
  let masked = "";
  for (let index = 0; index < source.length;) {
    const current = source[index];
    if (current === "/" && source[index + 1] === "/") {
      const end = source.indexOf("\n", index + 2);
      const next = end < 0 ? source.length : end;
      masked += blank(source, index, next);
      index = next;
    } else if (current === "/" && source[index + 1] === "*") {
      let depth = 1;
      let end = index + 2;
      while (end < source.length && depth > 0) {
        if (source[end] === "/" && source[end + 1] === "*") {
          depth++;
          end += 2;
        } else if (source[end] === "*" && source[end + 1] === "/") {
          depth--;
          end += 2;
        } else end++;
      }
      if (depth !== 0) throw new Error(`unterminated block comment at byte ${index}`);
      masked += blank(source, index, end);
      index = end;
    } else {
      const rawEnd = /[brc]/.test(current) ? rawStringEnd(source, index) : null;
      if (rawEnd !== null) {
        masked += blank(source, index, rawEnd);
        index = rawEnd;
      } else if ((current === "b" || current === "c") && source[index + 1] === '"') {
        const end = quotedStringEnd(source, index + 1);
        masked += blank(source, index, end);
        index = end;
      } else if (current === '"') {
        const end = quotedStringEnd(source, index);
        masked += blank(source, index, end);
        index = end;
      } else if (current === "b" && source[index + 1] === "'") {
        const end = charLiteralEnd(source, index + 1);
        masked += end === null ? current : blank(source, index, end);
        index = end ?? index + 1;
      } else if (current === "'") {
        const end = charLiteralEnd(source, index);
        masked += end === null ? current : blank(source, index, end);
        index = end ?? index + 1;
      } else {
        masked += current;
        index++;
      }
    }
  }
  return masked;
}

function bracePairs(masked) {
  const stack = [];
  const pairs = new Map();
  for (let index = 0; index < masked.length; index++) {
    if (masked[index] === "{") stack.push(index);
    else if (masked[index] === "}") {
      const open = stack.pop();
      if (open === undefined) throw new Error(`unmatched closing brace at byte ${index}`);
      pairs.set(open, index);
    }
  }
  if (stack.length) throw new Error(`unmatched opening brace at byte ${stack.at(-1)}`);
  return pairs;
}

export function inlineTestRanges(source, masked = maskRustTrivia(source)) {
  const pairs = bracePairs(masked);
  const ranges = [];
  const modulePattern =
    /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{/g;
  for (const match of masked.matchAll(modulePattern)) {
    const open = match.index + match[0].lastIndexOf("{");
    const close = pairs.get(open);
    if (close === undefined) throw new Error(`unclosed cfg(test) module at byte ${open}`);
    ranges.push({ start: open, end: close + 1 });
  }
  return ranges;
}

export function debugOnlyFunctionRanges(source, masked = maskRustTrivia(source)) {
  const pairs = bracePairs(masked);
  const ranges = [];
  const functionPattern =
    /#\s*\[\s*cfg\s*\(\s*debug_assertions\s*\)\s*\]\s*(?:pub(?:\s*\([^)]*\))?\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*(?:\s*<[^>]*>)?\s*\([^)]*\)[^{]*\{/g;
  for (const match of masked.matchAll(functionPattern)) {
    const open = match.index + match[0].lastIndexOf("{");
    const close = pairs.get(open);
    if (close === undefined) throw new Error(`unclosed debug-only function at byte ${open}`);
    ranges.push({ start: match.index, end: close + 1 });
  }
  return ranges;
}

export async function collectRustFiles(directory, result = []) {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) await collectRustFiles(entryPath, result);
    else if (entry.isFile() && entry.name.endsWith(".rs")) result.push(entryPath);
  }
  return result;
}
