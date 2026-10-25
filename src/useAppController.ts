import { currentScanRows } from './scanAssessment';
import { useToolTask } from './ToolCache';
import { useWindowsMaintenance } from './WindowsMaintenance';
import { uninstallActive } from './softwareTypes';
import { useUvTask } from './UvCache';
import { useElapsed } from './useElapsed';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api, desktop } from './api';
import { useApplicationSupport } from './ApplicationSupport';
import { ipcError, type CoordinationState } from './coordinator';
import { exclusionPath } from './ExclusionsDialog';
import { text as t, ui } from './i18n';
import { cleanupDecision } from './cleanupDecision';
import type {
  Bootstrap,
  Candidate,
  ExcludedFile,
  ExclusionDraft,
  Report,
  Rule,
  Scope,
  ScopedPlan,
  Settings,
} from './types';
import { useSettings } from './useSettings';
import { active, useTasks } from './useTasks';
import { useSpaceJourney } from './useSpaceJourney';
import { useManualPreview } from './useManualPreview';
import { useHomeSpace } from './useHomeSpace';
import { manualRequest, toggleManualBatch, type ManualChoices } from './manualSelection';

export function useAppController() {
  const [boot, setBoot] = useState<Bootstrap | null>(null);
  const windowsMaintenance = useWindowsMaintenance(!!boot, !!boot?.capabilities.system_cleanup);
  const [applicationEpoch, setApplicationEpoch] = useState(0);
  const applicationInfo = useApplicationSupport(!!boot, applicationEpoch);
  const [initEpoch, setInitEpoch] = useState(0);
  const [initializing, setInitializing] = useState(true);
  const [initError, setInitError] = useState('');
  const [checking, setChecking] = useState<CoordinationState>('ready');
  const [historyEpoch, setHistoryEpoch] = useState(0);
  const [historyError, setHistoryError] = useState('');
  const [historyLoading, setHistoryLoading] = useState(false);
  const [tab, setTab] = useState<'clean' | 'analysis' | 'software' | 'history' | 'settings'>(
    'clean',
  );
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [manualChoices, setManualChoices] = useState<ManualChoices>(new Map());
  const [scanVolumeSelection, setScanVolumeSelection] = useState<string[] | null>([]);
  const scanVolumesInitialized = useRef(false);
  const chooseScanVolumes = (tokens: string[] | null) => {
    scanVolumesInitialized.current = true;
    setScanVolumeSelection(tokens);
  };
  const [preparing, setPreparing] = useState(false);
  const preparationCancelled = useRef(false);
  const preparationId = useRef<string | null>(null);
  const [stoppingPreparation, setStoppingPreparation] = useState(false);
  const preparationElapsed = useElapsed(preparing);
  const [included, setIncluded] = useState<Map<string, ExcludedFile>>(new Map());
  const [reviewed, setReviewed] = useState<Set<string>>(new Set());
  const autoSelectSessions = useRef(new Set<string>());
  const scanRuleIds = (boot?.rules ?? [])
    .filter((r) => !r.requires_admin && !boot?.settings.excluded_rules.includes(r.id))
    .map((r) => r.id);
  const [history, setHistory] = useState<Report[]>([]);
  const [error, setError] = useState('');
  const {
    uninstall,
    analysis,
    invalidateAnalysis,
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
    retryStatus,
    reconcile,
  } = useTasks(setError);
  const maintenance = useToolTask(
    !!boot,
    sessions.find((s) => s.scope === 'user')?.id ?? '',
    summaries.user?.rules,
  );
  const uv = useUvTask(
    !!boot,
    sessions.find((s) => s.scope === 'user')?.id ?? '',
    summaries.user?.rules.find((r) => r.rule_id === 'dev.uv'),
  );
  const journey = useSpaceJourney(
    !!boot && !initializing && !initError,
    analysis,
    reports.map((r) => `${r.id}:${r.finished_at}:${r.status}`).join('|'),
    tab === 'analysis',
  );
  useEffect(() => {
    if (scanVolumesInitialized.current || !journey.locations.length) return;
    const volumes = journey.locations.filter((location) => location.kind === 'volume');
    if (!volumes.length) return;
    scanVolumesInitialized.current = true;
    const c = volumes.find(
      (location) => /^c:\\$/i.test(location.path) && location.supported !== false,
    );
    setScanVolumeSelection(c ? [c.token] : []);
  }, [journey.locations]);
  const scanVolumeTokens = scanVolumeSelection?.filter((token) =>
    journey.locations.some(
      (location) =>
        location.token === token && location.kind === 'volume' && location.supported !== false,
    ),
  );
  const canScanVolumes =
    journey.locationsLoaded &&
    !journey.locationsLoading &&
    !journey.locationError &&
    journey.locations.some((l) => l.kind === 'volume' && l.supported !== false) &&
    (scanVolumeSelection === null || !!scanVolumeTokens?.length);
  const homeSpace = useHomeSpace(
    journey.locations,
    scanVolumeSelection,
    reports.map((r) => `${r.id}:${r.finished_at}:${r.status}`).join('|'),
  );
  const [analysisTarget, setAnalysisTarget] = useState<{
    session: string;
    entry: import('./types').AnalysisEntry;
    key: number;
  } | null>(null);
  const [notice, setNotice] = useState('');
  const [handoffSession, setHandoffSession] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [closing, setClosing] = useState(false);
  const [plans, setPlans] = useState<ScopedPlan[]>([]);
  const [preparedSelection, setPreparedSelection] = useState<{
    files: number;
    directories: number;
    bytes: bigint;
    known: boolean;
  } | null>(null);
  useEffect(() => {
    setManualChoices(new Map());
    setPlans([]);
  }, [analysis?.id, analysis?.stale]);
  const manual = useMemo(
    () => (analysis && !analysis.stale ? manualRequest(analysis.id, manualChoices) : undefined),
    [analysis?.id, analysis?.stale, manualChoices],
  );
  const manualPreview = useManualPreview(manual, preparing);
  const hasManual = !!manual;
  function selectManual(entry: import('./types').AnalysisEntry, selected: boolean) {
    selectManualBatch([entry], selected);
  }
  function selectManualBatch(entries: import('./types').AnalysisEntry[], selected: boolean) {
    if (busy || saving || !analysis || analysis.stale) return;
    setManualChoices((choices) => toggleManualBatch(choices, entries, selected));
    setPlans([]);
  }
  function clearManual() {
    if (busy || saving) return;
    setManualChoices(new Map());
    setPlans([]);
  }
  function inspectManual(entry: import('./types').AnalysisEntry) {
    if (!analysis || analysis.stale) return;
    setAnalysisTarget((previous) => ({
      session: analysis.id,
      entry,
      key: (previous?.key ?? 0) + 1,
    }));
    setTab('analysis');
  }
  const [detail, setDetail] = useState<Rule | null>(null);
  const [detailQuery, setDetailQuery] = useState<{
    view: 'scan' | 'cleanup' | 'unplanned';
    status: string | null;
    session: string;
    reportId: string | null;
  }>({ view: 'scan', status: null, session: '', reportId: null });
  function openDetail(
    rule: Rule,
    view: 'scan' | 'cleanup' | 'unplanned' = reports.length ? 'cleanup' : 'scan',
    status: string | null = null,
    reportId?: string,
  ) {
    const scope = rule.requires_admin ? 'system' : 'user';
    const session = sessions.find((s) => s.scope === scope)?.id;
    const report = reports.find((r) => r.scope === scope);
    if (
      !session ||
      summaries[scope]?.session_id !== session ||
      jobs[scope]?.id !== session ||
      !summaries[scope]?.rules.some((r) => r.rule_id === rule.id) ||
      (view !== 'scan' && (!report || (reportId !== undefined && reportId !== report.id)))
    ) {
      setNotice('本次文件明细已不可用，保留的统计仍可在报告中核对；不会借用其他扫描会话。');
      return;
    }
    setDetailQuery({ view, status, session, reportId: view === 'scan' ? null : report!.id });
    setDetail(rule);
  }
  const detailScope = detail?.requires_admin ? 'system' : 'user';
  const detailCurrent =
    !!detail &&
    sessions.some((s) => s.scope === detailScope && s.id === detailQuery.session) &&
    jobs[detailScope]?.id === detailQuery.session &&
    summaries[detailScope]?.session_id === detailQuery.session &&
    !!summaries[detailScope]?.rules.some((r) => r.rule_id === detail.id) &&
    (detailQuery.reportId === null ||
      reports.some((r) => r.scope === detailScope && r.id === detailQuery.reportId));
  useEffect(() => {
    if (detail && !detailCurrent) setDetail(null);
  }, [detail, detailCurrent]);
  const [excluded, setExcluded] = useState<Map<string, ExcludedFile>>(new Map());
  const [expanded, setExpanded] = useState<Set<string>>(
    new Set(['windows', 'edge', 'chrome', 'firefox']),
  );
  const [clearDialog, setClearDialog] = useState(false);
  const [drafts, setDrafts] = useState<ExclusionDraft[]>([]);
  const [draftDialog, setDraftDialog] = useState(false);
  const [rescanDialog, setRescanDialog] = useState(false);
  const previousSession = useRef('');
  const previousScopes = useRef(new Map<string, string>());
  const sessionKey = sessions.map((s) => `${s.scope}:${s.id}`).join('|');
  useEffect(() => {
    if (previousSession.current !== sessionKey) {
      const unchanged = new Set(
        sessions.filter((s) => previousScopes.current.get(s.scope) === s.id).map((s) => s.scope),
      );
      const keep = (id: string) =>
        unchanged.has(boot?.rules.find((r) => r.id === id)?.requires_admin ? 'system' : 'user');
      setExcluded((previous) => new Map([...previous].filter(([, value]) => keep(value.rule_id))));
      setIncluded((previous) => new Map([...previous].filter(([, value]) => keep(value.rule_id))));
      setReviewed((previous) => new Set([...previous].filter(keep)));
      setSelected((previous) => new Set([...previous].filter(keep)));
      setPlans([]);
      setDetail(null);
      if (!unchanged.has('user')) setDrafts([]);
      previousScopes.current = new Map(sessions.map((s) => [s.scope, s.id]));
      previousSession.current = sessionKey;
    }
  }, [sessionKey]);
  const busy =
    windowsMaintenance.busy ||
    jobs.user?.rule_refresh?.state === 'scanning' ||
    maintenance.task?.status === 'running' ||
    uv.task?.status === 'running' ||
    initializing ||
    !!initError ||
    checking !== 'ready' ||
    pending ||
    closing ||
    uncertain ||
    stopping ||
    analysis?.state === 'scanning' ||
    uninstallActive(uninstall?.phase) ||
    Object.values(jobs).some((j) => active(j.state));
  const ready =
    !busy &&
    handoffSession !== sessionKey &&
    sessions.some((s) => jobs[s.scope]?.state === 'ready');
  const summaryRows = currentScanRows(sessions, summaries, jobs, handoffSession === sessionKey);
  const scanned = summaryRows.length > 0;
  const ruleRefresh = jobs.user?.rule_refresh;
  const refreshSubmission = useRef(false);
  const automaticRefreshes = useRef(new Set<string>());
  const [ruleRefreshError, setRuleRefreshError] = useState<Record<string, string>>({});
  useEffect(() => {
    setRuleRefreshError({});
  }, [sessionKey]);
  async function refreshRule(rule: string) {
    const session = sessions.find((s) => s.scope === 'user');
    if (!session || busy || saving || refreshSubmission.current) return;
    refreshSubmission.current = true;
    setPlans([]);
    setRuleRefreshError((v) => ({ ...v, [rule]: '' }));
    try {
      await api.refreshRule(session.id, rule);
    } catch (error) {
      setRuleRefreshError((v) => ({
        ...v,
        [rule]: `检查请求未确认，请查询任务状态；不会自动重试。${String(error)}`,
      }));
    } finally {
      await reconcile();
      refreshSubmission.current = false;
    }
  }
  async function cancelRuleRefresh() {
    const session = sessions.find((s) => s.scope === 'user');
    if (!session || ruleRefresh?.state !== 'scanning') return;
    try {
      await api.cancel([session], ruleRefresh.id);
    } catch (error) {
      setRuleRefreshError((v) => ({ ...v, [ruleRefresh.rule_id]: String(error) }));
    } finally {
      await reconcile();
    }
  }
  useEffect(() => {
    if (desktop && (uv.task || maintenance.task)) void reconcile();
  }, [uv.task?.id, uv.task?.status, maintenance.task?.id, maintenance.task?.status, reconcile]);

  const currentJob = Object.values(jobs).find((j) => active(j.state));
  const decision = cleanupDecision({
    rules: boot?.rules ?? [],
    applications: applicationInfo.applications,
    rows: summaryRows,
    selected,
    excluded,
    included,
    excludedRuleIds: boot?.settings.excluded_rules,
    sessions,
    reports,
    canPlan:
      handoffSession !== sessionKey && sessions.some((s) => jobs[s.scope]?.state === 'ready'),
    working: busy,
    needsRescan: handoffSession === sessionKey,
    uncertain: uncertain || checking !== 'ready',
    stopping,
    reviewContext: { jobs, summaries },
    active:
      currentJob?.state === 'scanning'
        ? 'scanning'
        : currentJob?.state === 'cleaning'
          ? 'cleaning'
          : analysis?.state === 'scanning'
            ? 'analysis'
            : null,
  });
  const totals = decision.totals;
  if (hasManual && (!busy || preparing)) {
    decision.action = 'prepare';
    decision.actionLabel = preparing ? '正在准备…' : '清理已选';
    decision.message = preparing
      ? '正在核验所选内容，可停止准备。'
      : '已加入自选项目，确认前会核验可删除的内容。';
  }
  if (windowsMaintenance.selected.size && !busy) {
    decision.canPrepare = true;
    decision.action = 'prepare';
    decision.actionLabel = '清理已选';
    decision.message = '已选文件与 Windows 维护项目将分别核验，确认后依次执行。';
  }
  const eligible = summaryRows.filter(
    (s) => !s.blocked && (totals.byRule.get(s.rule_id)?.files ?? 0) > 0,
  );
  const recommendedIds = decision.items
    .filter(
      (r) =>
        r.rule.cleanup_policy === 'recommended' &&
        r.capability === 'allowed' &&
        ['ready', 'deselected'].includes(r.status) &&
        r.foundFiles > 0 &&
        !boot?.settings.excluded_rules.includes(r.rule.id),
    )
    .map((r) => r.rule.id);
  useEffect(() => {
    if (!ready || reports.length || !sessions.some((s) => autoSelectSessions.current.has(s.id)))
      return;
    // Initialize once per newly requested scan. Availability refresh never restores a choice.
    sessions.forEach((s) => autoSelectSessions.current.delete(s.id));
    setSelected(new Set(recommendedIds));
  }, [ready, reports.length, sessionKey, summaries]);
  const executableChoices = decision.items.filter(
    (item) => item.capability === 'allowed' && ['ready', 'deselected'].includes(item.status),
  );
  const candidateBytes = executableChoices.reduce((bytes, item) => bytes + item.bytes, 0n);
  const candidateCount = executableChoices.reduce((files, item) => files + item.files, 0);
  const selectionEstimate = {
    bytes:
      candidateBytes + BigInt(hasManual && manualPreview.data ? manualPreview.data.bytes : '0'),
    files: candidateCount + (hasManual && manualPreview.data ? manualPreview.data.files : 0),
    directories: hasManual && manualPreview.data ? manualPreview.data.directories : 0,
    known: !hasManual || !!manualPreview.data,
  };
  const selectionVersion = useMemo(
    () => ({}),
    [
      selected,
      included,
      excluded,
      reviewed,
      manualChoices,
      sessionKey,
      windowsMaintenance.selected,
      analysis?.id,
      analysis?.stale,
    ],
  );
  const latestSelection = useRef(selectionVersion);
  latestSelection.current = selectionVersion;
  useEffect(() => {
    setPlans([]);
    setPreparedSelection(null);
    windowsMaintenance.setPlans([]);
  }, [selectionVersion]);
  const emptySelection = decision.message;

  useEffect(() => {
    let valid = true;
    setInitializing(true);
    setInitError('');
    setPlans([]);
    (async () => {
      if (desktop) await api.attach();
      return api.bootstrap();
    })()
      .then(async (b) => {
        if (valid) {
          setBoot(b);
          setSelected(new Set());
          setIncluded(new Map());
          setReviewed(new Set());
          autoSelectSessions.current.clear();
          if (desktop) {
            const snapshots = await reconcile(['user']);
            if (valid && snapshots?.some((s) => s.status.id)) {
              setSelected(new Set());
              setNotice(
                snapshots.some((s) => s.report)
                  ? '已恢复上次清理记录。'
                  : snapshots.some((s) => ['scanning', 'cleaning'].includes(s.status.state))
                    ? '已恢复进行中的任务。'
                    : '已恢复扫描结果，请选择本次要清理的内容。',
              );
            }
          }
        }
      })
      .catch((e) => {
        if (valid) setInitError(String(e));
      })
      .finally(() => {
        if (valid) setInitializing(false);
      });
    return () => {
      valid = false;
    };
  }, [reconcile, initEpoch]);
  useEffect(() => api.onCoordination(setChecking), []);
  useEffect(() => {
    document.documentElement.dataset.theme = boot?.settings.theme ?? 'system';
  }, [boot?.settings.theme]);
  useEffect(() => {
    if (!desktop) return;
    const registration = listen('closing', () => setClosing(true));
    return () => {
      registration.then((unlisten) => unlisten());
    };
  }, []);

  const guard = useCallback(async (operation: () => Promise<unknown>) => {
    setPending(true);
    setError('');
    try {
      await operation();
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }, []);
  const stopTask = () =>
    void guard(async () => {
      setStopping(true);
      generation.current++;
      try {
        if (analysis?.state === 'scanning') await api.cancelAnalysis(analysis.id);
        else await api.cancel(sessions);
      } catch (e) {
        setError(`停止请求响应异常，正在核对状态。${String(e)}`);
      }
      await reconcile();
    });
  useEffect(() => {
    let valid = true;
    if (tab === 'history') {
      setHistoryError('');
      setHistoryLoading(true);
      api
        .history()
        .then((value) => {
          if (valid) setHistory(value);
        })
        .catch((e) => {
          if (valid) setHistoryError(String(e));
        })
        .finally(() => {
          if (valid) setHistoryLoading(false);
        });
    }
    return () => {
      valid = false;
    };
  }, [tab, reports, historyEpoch]);

  function toggle(id: string, chosen = !selected.has(id)) {
    const item = decision.items.find((r) => r.rule.id === id);
    if (
      busy ||
      !item ||
      !['ready', 'deselected'].includes(item.status) ||
      item.capability !== 'allowed' ||
      boot?.settings.excluded_rules.includes(id)
    )
      return;
    setPlans([]);
    setIncluded((prev) => new Map([...prev].filter(([, v]) => v.rule_id !== id)));
    setExcluded((prev) => new Map([...prev].filter(([, v]) => v.rule_id !== id)));
    setReviewed((prev) => {
      const next = new Set(prev);
      if (!chosen) next.delete(id);
      else if (item.rule.cleanup_policy !== 'recommended') next.add(id);
      return next;
    });
    setSelected((prev) => {
      const next = new Set(prev);
      if (!chosen) next.delete(id);
      else next.add(id);
      return next;
    });
  }
  function selectRuleGroup(ids: string[], policy: 'recommended' | 'review', chosen: boolean) {
    if (!ready || busy || saving) return;
    const allowed = new Set(
      decision.items
        .filter(
          (item) =>
            ids.includes(item.rule.id) &&
            item.capability === 'allowed' &&
            ['ready', 'deselected'].includes(item.status) &&
            item.foundFiles > 0 &&
            (item.rule.cleanup_policy === 'recommended' ? 'recommended' : 'review') === policy &&
            !boot?.settings.excluded_rules.includes(item.rule.id),
        )
        .map((item) => item.rule.id),
    );
    if (!allowed.size) return;
    setPlans([]);
    setIncluded(
      (previous) => new Map([...previous].filter(([, item]) => !allowed.has(item.rule_id))),
    );
    setExcluded(
      (previous) => new Map([...previous].filter(([, item]) => !allowed.has(item.rule_id))),
    );
    setReviewed((previous) => {
      const next = new Set(previous);
      for (const id of allowed) {
        if (chosen && policy === 'review') next.add(id);
        else next.delete(id);
      }
      return next;
    });
    setSelected((previous) => {
      const next = new Set(previous);
      for (const id of allowed) chosen ? next.add(id) : next.delete(id);
      return next;
    });
  }
  function selectFiles(files: Candidate[], chosen: boolean, scope: Scope, session: string) {
    if (!ready || !sessions.some((s) => s.scope === scope && s.id === session)) return;
    files = files.filter((f) => {
      const item = decision.items.find((r) => r.rule.id === f.rule_id);
      return (
        f.status === 'candidate' &&
        item?.capability === 'allowed' &&
        ['ready', 'deselected'].includes(item.status) &&
        !boot?.settings.excluded_rules.includes(f.rule_id)
      );
    });
    setPlans([]);
    setIncluded((prev) => {
      const next = new Map(prev);
      for (const file of files) {
        if (chosen && !selected.has(file.rule_id))
          next.set(file.id, { scope, session, rule_id: file.rule_id, bytes: file.bytes, file });
        else next.delete(file.id);
      }
      return next;
    });
    if (chosen)
      setReviewed(
        (prev) =>
          new Set([
            ...prev,
            ...files
              .filter(
                (f) =>
                  boot?.rules.find((r) => r.id === f.rule_id)?.cleanup_policy !== 'recommended',
              )
              .map((f) => f.rule_id),
          ]),
      );
    setExcluded((prev) => {
      const next = new Map(prev);
      for (const file of files) {
        if (chosen) next.delete(file.id);
        else if (selected.has(file.rule_id))
          next.set(file.id, { scope, session, rule_id: file.rule_id, bytes: file.bytes, file });
      }
      return next;
    });
  }
  function selectRecommended(ids?: string[], chosen = true) {
    if (!ready || busy) return;
    setPlans([]);
    const allowed = ids ? recommendedIds.filter((id) => ids.includes(id)) : recommendedIds;
    if (!ids) {
      setSelected(new Set(allowed));
      setIncluded(new Map());
      setExcluded(new Map());
      setReviewed(new Set());
    } else {
      setSelected((prev) => {
        const next = new Set(prev);
        allowed.forEach((id) => (chosen ? next.add(id) : next.delete(id)));
        return next;
      });
      setIncluded((prev) => new Map([...prev].filter(([, v]) => !allowed.includes(v.rule_id))));
      setExcluded((prev) => new Map([...prev].filter(([, v]) => !allowed.includes(v.rule_id))));
    }
  }
  async function refreshAvailability() {
    if (busy || saving) return;
    setNotice('');
    setApplicationEpoch((v) => v + 1);
    setPlans([]);
    await guard(async () => {
      const token = generation.current;
      const results = await Promise.all(
        sessions.map(async (s) => ({ scope: s.scope, summary: await api.refresh(s.scope, s.id) })),
      );
      if (token !== generation.current) return;
      // A snapshot taken before the availability refresh must not restore the old block.
      generation.current++;
      setSummaries(Object.fromEntries(results.map((r) => [r.scope, r.summary])));
      const rescan = results.some(({ summary }) =>
        summary.rules.some(
          (row) => row.scan_failed || row.coverage?.complete === false || row.files === 0,
        ),
      );
      setNotice(
        rescan
          ? '应用状态已重新检查，原有候选和选择已保留。复查不会查找新文件；零候选或未读完整的位置请重新扫描。'
          : '应用状态已重新检查，原有候选和选择已保留，可继续核验已选内容；查找新增文件请重新扫描。',
      );
    });
  }
  async function startScan(ids: string[]) {
    if (!canScanVolumes) {
      setError('请至少选择一个可用磁盘。');
      return 'not_started';
    }
    generation.current++;
    setPlans([]);
    const before = sessionKey;
    try {
      const result =
        scanVolumeSelection === null ? await api.scan(ids) : await api.scan(ids, scanVolumeTokens);
      result.sessions.forEach((s) => autoSelectSessions.current.add(s.id));
      if (result.warning) setNotice(result.warning);
      const snapshots = await reconcile(result.sessions.map((s) => s.scope));
      if (!snapshots) return 'unknown';
      return 'started';
    } catch (error) {
      const snapshots = await reconcile();
      if (snapshots) {
        const changed = snapshots.some((s) => s.status.id && !before.includes(s.status.id));
        if (changed)
          snapshots.forEach((s) => {
            if (s.status.id && !before.includes(s.status.id))
              autoSelectSessions.current.add(s.status.id);
          });
        setError(
          changed ? '扫描请求响应中断，已重新接上后台任务。' : `扫描未开始：${String(error)}`,
        );
        return changed ? 'started' : 'not_started';
      }
      return 'unknown';
    }
  }
  async function scan() {
    if (drafts.length) {
      setRescanDialog(true);
      return;
    }
    await guard(() => startScan(scanRuleIds));
  }
  async function scanSystem() {
    const ids = (boot?.rules ?? [])
      .filter((r) => r.requires_admin && !boot?.settings.excluded_rules.includes(r.id))
      .map((r) => r.id);
    if (!boot?.capabilities.system_cleanup || !ids.length) return;
    await guard(async () => {
      const result = await api.scan(ids);
      if (result.warning) setNotice(result.warning);
      if (result.sessions.length) {
        generation.current++;
        await reconcile(['user', 'system']);
      }
    });
  }
  async function prepare() {
    if ((!decision.canPrepare && !hasManual) || busy || saving) return;
    await guard(async () => {
      const version = latestSelection.current;
      setNotice('');
      setPreparing(true);
      preparationCancelled.current = false;
      setStoppingPreparation(false);
      const operationId = crypto.randomUUID();
      preparationId.current = operationId;
      try {
        const requests = sessions
          .filter((s) => jobs[s.scope]?.state === 'ready')
          .map((s) => ({
            scope: s.scope,
            session: s.id,
            selection: {
              rule_ids: eligible
                .filter(
                  (r) =>
                    boot?.rules.find((rule) => rule.id === r.rule_id)?.requires_admin ===
                      (s.scope === 'system') &&
                    selected.has(r.rule_id) &&
                    r.files > 0,
                )
                .map((r) => r.rule_id),
              excluded_ids: [...excluded]
                .filter(([, e]) => e.scope === s.scope && e.session === s.id)
                .map(([id]) => id),
              included_ids: [...included]
                .filter(
                  ([, e]) =>
                    e.scope === s.scope &&
                    e.session === s.id &&
                    eligible.some((r) => r.rule_id === e.rule_id),
                )
                .map(([id]) => id),
              reviewed_rule_ids: [...reviewed].filter((id) =>
                eligible.some((r) => r.rule_id === id),
              ),
            },
          }))
          .filter((r) => r.selection.rule_ids.length || r.selection.included_ids.length);
        const prepared =
          requests.length || manual ? await api.prepare(requests, manual, operationId) : [];
        const official = preparationCancelled.current ? [] : await windowsMaintenance.prepare();
        if (preparationCancelled.current) setNotice('准备已结束，选择已保留；没有执行删除。');
        if (!preparationCancelled.current && latestSelection.current === version) {
          const nonempty = prepared.filter(
            ({ plan }) => plan.files > 0 || (plan.manual?.directories ?? 0) > 0,
          );
          setPlans(nonempty);
          windowsMaintenance.setPlans(official);
          setPreparedSelection(selectionEstimate);
          if (!nonempty.length && !official.length)
            setNotice('核验后没有可删除的内容，所选项目已保留。可查看原因或重新扫描。');
        }
      } catch (error) {
        if (!preparationCancelled.current || ipcError(error).code !== 'cancelled') throw error;
        setNotice('已停止准备，选择已保留；没有执行删除。');
      } finally {
        preparationId.current = null;
        setPreparing(false);
        setStoppingPreparation(false);
      }
    });
  }
  async function execute() {
    if ((!plans.length && !windowsMaintenance.plans.length) || busy || saving) return;
    const confirmed = plans;
    const official = windowsMaintenance.plans;
    setPlans([]);
    windowsMaintenance.setPlans([]);
    windowsMaintenance.setSelected(new Set());
    await guard(async () => {
      generation.current++;
      try {
        const result = await api.execute(confirmed, official);
        if (result.warning) setNotice(result.warning);
      } catch (error) {
        setError(`清理请求响应异常，正在核对后台状态；不会重复提交。${String(error)}`);
      }
      await reconcile();
    });
  }
  const {
    saving,
    save,
    run: saveOperation,
  } = useSettings(
    (settings: Settings, invalidated: boolean) => {
      setBoot((b) => (b ? { ...b, settings } : b));
      if (invalidated) {
        generation.current++;
        invalidateAnalysis();
        setManualChoices(new Map());
        setDrafts([]);
        setDetail(null);
        setSessions([]);
        setJobs({});
        setSummaries({});
        setPlans([]);
        setReports([]);
        setExcluded(new Map());
        setIncluded(new Map());
        setReviewed(new Set());
        setSelected(
          (prev) => new Set([...prev].filter((id) => !settings.excluded_rules.includes(id))),
        );
      }
      setNotice(invalidated ? ui.settingsSaved : '外观已保存，扫描结果与选择保持不变。');
    },
    setError,
    async () => {
      const snapshot = await reconcile();
      if (snapshot) {
        const latest = await api.bootstrap();
        setBoot(latest);
        setSelected(
          (prev) => new Set([...prev].filter((id) => !latest.settings.excluded_rules.includes(id))),
        );
      }
    },
  );
  useEffect(() => {
    if (busy || saving || uv.error || maintenance.error || !api.refreshRule) return;
    for (const row of summaries.user?.rules ?? []) {
      const task = row.rule_id === 'dev.uv' ? uv.task : maintenance.results[row.rule_id];
      if (
        !row.pending_tool_task ||
        !task ||
        task.id !== row.pending_tool_task ||
        task.session !== summaries.user?.session_id ||
        !task.finished_at ||
        task.status === 'running' ||
        automaticRefreshes.current.has(task.id)
      )
        continue;
      automaticRefreshes.current.add(task.id);
      void refreshRule(row.rule_id);
      break;
    }
  }, [busy, saving, summaries.user, uv.task, uv.error, maintenance.results, maintenance.error]);
  async function addExclusion(kind: 'file' | 'folder') {
    if (!boot) return;
    await guard(async () => {
      const path = await api.pick(kind);
      if (path && !boot.settings.exclusions.includes(path))
        await save({ exclusions: [...boot.settings.exclusions, path] });
    });
  }
  function addDraft(files: Candidate[], kind: 'file' | 'parent', scope: Scope, session: string) {
    if (scope !== 'user' || busy || saving) return;
    setDrafts((prev) => {
      const next = [...prev];
      for (const file of files) {
        const item = { scope, session, file, kind };
        if (
          !next.some(
            (d) =>
              d.session === session &&
              exclusionPath(d).toLowerCase() === exclusionPath(item).toLowerCase(),
          )
        )
          next.push(item);
      }
      if (next.length > 500) {
        queueMicrotask(() => setError('排除草稿最多 500 项，请减少本类别取消的文件数量后再添加。'));
        return prev;
      }
      return next;
    });
  }
  async function saveDrafts() {
    if (!drafts.length || busy || saving) return;
    const currentDrafts = drafts;
    const ids = scanRuleIds;
    await guard(async () => {
      const result = await saveOperation(() =>
        api.addExclusions(
          'user',
          currentDrafts[0].session,
          currentDrafts.map((d) => ({ candidate_id: d.file.id, kind: d.kind })),
        ),
      );
      if (!result) {
        return;
      }
      setDrafts([]);
      setDraftDialog(false);
      setRescanDialog(false);
      const allowed = ids.filter((id) => !result.settings.excluded_rules.includes(id));
      if (!allowed.length) {
        setNotice('排除已保存，尚未选择扫描类别。');
        return;
      }
      const outcome = await startScan(allowed);
      if (outcome === 'not_started') setNotice('排除已保存，扫描未开始。请选择类别后重新扫描。');
      else if (outcome === 'started') setNotice('排除已保存，已重新核对扫描状态。');
      else setNotice('排除已保存，暂时无法确认后续扫描状态，请只读重查。');
    });
  }
  const title =
    currentJob?.state === 'scanning'
      ? t.scanning
      : currentJob?.state === 'cleaning'
        ? t.cleaning
        : reports.length
          ? ui.resultTitle
          : ready
            ? candidateCount > 0
              ? ui.readyTitle
              : summaryRows.some((s) => s.phase !== 'completed')
                ? '扫描结束 · 结果不完整'
                : '扫描完成'
            : t.title;

  return {
    selectionEstimate,
    homeSpace,
    manualPreview,
    analysisTarget,
    inspectManual,
    selectManualBatch,
    scanVolumeSelection,
    setScanVolumeSelection: chooseScanVolumes,
    canScanVolumes,
    excludeManual: (entry: import('./types').AnalysisEntry) =>
      guard(async () => {
        if (!analysis || busy || saving) return;
        await api.excludeAnalysis(analysis.id, entry.id);
        setBoot(await api.bootstrap());
        setManualChoices(new Map());
        setPlans([]);
        await reconcile();
        setNotice('已长期保留此项；重新扫描后会按排除设置处理。');
      }),
    manualChoices,
    selectManual,
    hasManual,
    preparing,
    stoppingPreparation,
    preparationElapsed,
    clearManual,
    cancelPreparation: async () => {
      const id = preparationId.current;
      if (!id || preparationCancelled.current) return;
      preparationCancelled.current = true;
      setStoppingPreparation(true);
      setPlans([]);
      setNotice('正在停止准备，等待当前检查返回；选择会保留。');
      if (windowsMaintenance.busy) await windowsMaintenance.cancel();
      await api.cancelPreparation(id).catch((e) => {
        if (preparationId.current === id)
          setError(`停止请求未确认，仍在等待准备结果。${String(e)}`);
      });
    },
    decision,
    uv,
    maintenance,
    journey,
    boot,
    setBoot,
    uninstall,
    applicationEpoch,
    setApplicationEpoch,
    applicationInfo,
    initEpoch,
    setInitEpoch,
    initializing,
    setInitializing,
    initError,
    setInitError,
    checking,
    setChecking,
    historyEpoch,
    setHistoryEpoch,
    historyError,
    setHistoryError,
    historyLoading,
    setHistoryLoading,
    tab,
    setTab,
    selected,
    setSelected,
    included,
    reviewed,
    scanRuleIds,
    selectRecommended,
    selectRuleGroup,
    history,
    setHistory,
    error,
    setError,
    analysis,
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
    retryStatus,
    reconcile,
    notice,
    setNotice,
    handoffSession,
    setHandoffSession,
    pending,
    setPending,
    closing,
    setClosing,
    plans,
    preparedSelection,
    setPlans,
    detail,
    setDetail,
    detailQuery,
    detailCurrent,
    setDetailQuery,
    openDetail,
    excluded,
    setExcluded,
    expanded,
    setExpanded,
    clearDialog,
    setClearDialog,
    drafts,
    setDrafts,
    draftDialog,
    setDraftDialog,
    rescanDialog,
    setRescanDialog,
    previousSession,
    sessionKey,
    busy,
    ready,
    summaryRows,
    scanned,
    eligible,
    totals,
    candidateBytes,
    candidateCount,
    emptySelection,
    guard,
    stopTask,
    toggle,
    selectFiles,
    refreshAvailability,
    refreshRule,
    cancelRuleRefresh,
    ruleRefresh,
    ruleRefreshError,
    startScan,
    scanSystem,
    windowsMaintenance,
    scan,
    prepare,
    execute,
    saving,
    save,
    saveOperation,
    addExclusion,
    addDraft,
    saveDrafts,
    currentJob,
    title,
  };
}
export type AppController = ReturnType<typeof useAppController>;
