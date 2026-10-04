import { useEffect, useState } from 'react';

/** UI elapsed time only: no backend polling or invented percentage. */
export function useElapsed(active: boolean, identity?: unknown) {
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    setSeconds(0);
    if (!active) return;
    const started = Date.now();
    const timer = setInterval(() => setSeconds(Math.floor((Date.now() - started) / 1000)), 1000);
    return () => clearInterval(timer);
  }, [active, identity]);
  return active && seconds >= 2 ? seconds : 0;
}
