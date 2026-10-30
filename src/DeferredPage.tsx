import { Suspense, useEffect, useState, type ReactNode } from 'react';

/** Mount on first visit, then keep the page and its query/selection state alive. */
export function DeferredPage({ active, children }: { active: boolean; children: ReactNode }) {
  const [visited, setVisited] = useState(active);
  useEffect(() => {
    if (active) setVisited(true);
  }, [active]);
  if (!active && !visited) return null;
  return <Suspense fallback={<p role="status">正在打开…</p>}>{children}</Suspense>;
}
