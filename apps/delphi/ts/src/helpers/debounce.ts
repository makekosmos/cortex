export default function debounce<A extends unknown[]>(
  func: (...args: A) => void | Promise<void>,
  ms: number,
) {
  let timer: ReturnType<typeof setTimeout> | null = null;

  return (...args: A) => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void func(...args), ms);
  };
}
