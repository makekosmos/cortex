/** biome-ignore-all lint/suspicious/noArrayIndexKey: mocked */
import Skeleton from '@/components/skeleton';

export default function MockTaskList() {
  return (
    <ul className="space-y-2">
      {Array.from({ length: 6 }).map((_, i) => (
        <li
          key={i}
          className="flex w-full gap-5 border-2 border-(--background) px-7 py-3.5"
        >
          <Skeleton className="h-6 w-6 shrink-0 rounded-full bg-(--secondary)" />

          <div className="flex w-12 min-w-0 gap-2.5">
            <Skeleton className="h-full w-full bg-(--secondary)" />
          </div>

          <div className="flex w-full min-w-0 gap-2.5">
            <Skeleton className="h-full w-full bg-(--secondary)" />
          </div>
        </li>
      ))}
    </ul>
  );
}
