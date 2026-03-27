import type { ReactNode } from 'react';

export default function InfoCard({ children }: { children: ReactNode }) {
  return (
    <div className="my-5 rounded-xl bg-(--card) p-7.5 text-sm text-(--foreground)/70">
      {children}
    </div>
  );
}
