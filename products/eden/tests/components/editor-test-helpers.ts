export function makeEntry(contentJson: Record<string, unknown>): Entry {
  return {
    id: "cm-test-1",
    title: "Тест",
    content_json: JSON.stringify(contentJson),
    created_at: Date.now(),
    updated_at: Date.now(),
    folder_id: null,
    type_id: null,
    header_layout: null,
    header_props_json: "{}",
    schema_version: 1,
    deleted_at: null,
  };
}

export const EMPTY_DOC = { type: "doc", content: [{ type: "paragraph" }] };

export function markdownEntry(text: string): Entry {
  return makeEntry({ type: "markdown", version: 1, text });
}

export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}
