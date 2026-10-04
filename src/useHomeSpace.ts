import { useEffect, useRef, useState } from 'react';
import { api } from './api';
import { ipcError } from './coordinator';
import type { AnalysisContext, AnalysisLocation } from './types';

export interface DiskReading {
  context: AnalysisContext | null;
  loading: boolean;
  error: string;
}

/** One queue across refreshes: at most two native space queries, no polling. */
export function useHomeSpace(
  locations: AnalysisLocation[],
  selection: string[] | null,
  reportKey: string,
) {
  const volumes = locations.filter(
    (l) =>
      l.kind === 'volume' &&
      l.supported !== false &&
      (selection === null || selection.includes(l.token)),
  );
  const key = JSON.stringify(volumes.map((v) => v.token));
  const [epoch, setEpoch] = useState(0);
  const [readings, setReadings] = useState<Map<string, DiskReading>>(new Map());
  const cache = useRef(new Map<string, AnalysisContext>());
  const queue = useRef<{ token: string; generation: number }[]>([]);
  const generation = useRef(0);
  const active = useRef(0);
  const alive = useRef(true);
  const pump = useRef<() => void>(() => {});
  pump.current = () => {
    while (alive.current && active.current < 2 && queue.current.length) {
      const work = queue.current.shift()!;
      active.current++;
      void api
        .analysisContext(work.token)
        .then((context) => {
          if (!alive.current || generation.current !== work.generation) return;
          if (context.location_token !== work.token) throw new Error('磁盘读数与所选位置不一致');
          if (context.error || context.available_bytes === null || !context.volume_key)
            throw new Error(context.error || '未取得磁盘空间');
          cache.current.set(work.token, context);
          setReadings((old) =>
            new Map(old).set(work.token, { context, loading: false, error: '' }),
          );
        })
        .catch((error) => {
          if (alive.current && generation.current === work.generation)
            setReadings((old) =>
              new Map(old).set(work.token, {
                context: cache.current.get(work.token) ?? null,
                loading: false,
                error: ipcError(error).message,
              }),
            );
        })
        .finally(() => {
          active.current--;
          pump.current();
        });
    }
  };
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      generation.current++;
      queue.current = [];
    };
  }, []);
  useEffect(() => {
    const current = ++generation.current;
    const tokens: string[] = JSON.parse(key);
    queue.current = tokens.map((token) => ({ token, generation: current }));
    setReadings(
      new Map(
        tokens.map((token) => [
          token,
          {
            context: cache.current.get(token) ?? null,
            loading: true,
            error: '',
          },
        ]),
      ),
    );
    pump.current();
    return () => {
      generation.current++;
      queue.current = [];
    };
  }, [key, reportKey, epoch]);
  return { volumes, readings, refresh: () => setEpoch((n) => n + 1) };
}
