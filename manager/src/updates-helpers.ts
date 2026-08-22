export async function updateSequentially<T>(
  items: T[],
  update: (item: T) => Promise<string | null>,
): Promise<string[]> {
  const failures: string[] = [];
  for (const item of items) {
    const failure = await update(item);
    if (failure) failures.push(failure);
  }
  return failures;
}
