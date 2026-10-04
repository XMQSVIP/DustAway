import { uninstallActive, type UninstallTask } from './softwareTypes';
import { useCallback, useEffect, useRef, useState } from 'react';
import { api, desktop } from './api';
import type { Job, Report, Scope, Session, Summary, RuntimeSnapshot } from './types';
export const active = (state?: string) => state === 'scanning' || state === 'cleaning';

export function useTasks(onError: (message: string) => void) {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [jobs, setJobs] = useState<Partial<Record<Scope, Job>>>({});
  const [summaries, setSummaries] = useState<Partial<Record<Scope, Summary>>>({});
  const [reports, setReports] = useState<Report[]>([]);
  const [uninstall, setUninstall] = useState<UninstallTask | null>(null);
  const [analysis, setAnalysis] = useState<import('./types').AnalysisSnapshot | null>(null);
  const [uncertain, setUncertain] = useState(desktop);
  const [stopping, setStopping] = useState(false);
  const [pollEpoch, setPollEpoch] = useState(0);
  const generation = useRef(0);
  const sequence = useRef(0);
  const current = useRef(sessions);
  current.current = sessions;
  const read = useCallback(
    async (scopes: Scope[]) => {
      const request = ++sequence.current;
      const token = generation.current;
      try {
        const updates = await Promise.all(
          scopes.map(async (scope) => ({ scope, snapshot: await api.snapshot(scope) })),
        );
        if (request !== sequence.current || token !== generation.current) return null;
        for (const { snapshot } of updates) {
          if (
            snapshot.status.id !== snapshot.summary.session_id ||
            snapshot.status.state !== snapshot.summary.state
          )
            throw new Error('后台快照不一致，请重新检查');
        }
        setUncertain(false);
        const user = updates.find((u) => u.scope === 'user');
        if (user) {
          setAnalysis(user.snapshot.analysis ?? null);
          setUninstall(user.snapshot.uninstall ?? null);
        }
        setJobs(Object.fromEntries(updates.map(({ scope, snapshot }) => [scope, snapshot.status])));
        setSummaries(
          Object.fromEntries(updates.map(({ scope, snapshot }) => [scope, snapshot.summary])),
        );
        const next = updates
          .filter((u) => u.snapshot.status.id)
          .map((u) => ({ scope: u.scope, id: u.snapshot.status.id }));
        setSessions((prev) => (JSON.stringify(prev) === JSON.stringify(next) ? prev : next));
        const running = updates.some(
          (u) =>
            active(u.snapshot.status.state) ||
            u.snapshot.status.rule_refresh?.state === 'scanning' ||
            u.snapshot.analysis?.state === 'scanning' ||
            uninstallActive(u.snapshot.uninstall?.phase),
        );
        if (!running) setStopping(false);
        setReports(
          updates.flatMap((u) =>
            !active(u.snapshot.status.state) && u.snapshot.report ? [u.snapshot.report] : [],
          ),
        );
        for (const u of updates) if (u.snapshot.status.error) onError(u.snapshot.status.error);
        return updates.map((u) => u.snapshot);
      } catch (error) {
        if (request !== sequence.current || token !== generation.current) return null;
        setUncertain(true);
        onError(`暂时无法确认任务状态。请重新检查，勿重复开始任务。${String(error)}`);
        return null;
      }
    },
    [onError],
  );
  const reconcile = useCallback(
    async (scopes?: Scope[]): Promise<RuntimeSnapshot[] | null> => {
      setUncertain(true);
      const result = await read(
        scopes ?? [...new Set<Scope>(['user', ...current.current.map((s) => s.scope)])],
      );
      if (result) setPollEpoch((v) => v + 1);
      return result;
    },
    [read],
  );
  useEffect(() => {
    if (!sessions.length && !analysis && !uninstall) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    async function tick() {
      const snapshots = await read([...new Set<Scope>(['user', ...sessions.map((s) => s.scope)])]);
      if (
        !stopped &&
        snapshots?.some(
          (s) =>
            active(s.status.state) ||
            s.status.rule_refresh?.state === 'scanning' ||
            s.analysis?.state === 'scanning' ||
            uninstallActive(s.uninstall?.phase),
        )
      )
        timer = setTimeout(tick, 500);
    }
    // A reconcile already supplies the first snapshot.
    timer = setTimeout(tick, 500);
    return () => {
      stopped = true;
      clearTimeout(timer);
      sequence.current++;
    };
  }, [sessions, pollEpoch, read, analysis?.id, analysis?.state, uninstall?.id, uninstall?.phase]);
  useEffect(
    () => () => {
      generation.current++;
      sequence.current++;
    },
    [],
  );
  return {
    uninstall,
    analysis,
    invalidateAnalysis: () => setAnalysis((value) => (value ? { ...value, stale: true } : null)),
    sessions,
    setSessions,
    jobs,
    setJobs,
    summaries,
    setSummaries,
    reports,
    setReports,
    uncertain,
    stopping,
    setStopping,
    generation,
    reconcile,
    retryStatus: () => {
      void reconcile();
    },
  };
}
