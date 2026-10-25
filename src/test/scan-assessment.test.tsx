import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';
import { assessScan, currentScanRows, scanAccountingFor, scanAmount } from '../scanAssessment';
import { cleanupDecision } from '../cleanupDecision';
import { isSelectableCleanup, ruleFollowup } from '../cleanupPresentation';
import { ScanTotals } from '../ScanTotals';
import { PurposeGroups } from '../PurposeGroups';
import { CleanupRuleList } from '../CleanupRuleList';
import { WeixinCleanup } from '../WeixinCleanup';
import rules from '../previewRules';
import type { AppController } from '../useAppController';
import type { Job, RuleSummary, Summary } from '../types';
afterEach(cleanup);
const readonly = 'appcache.codex_logs';
const allowed = 'windows.user_temp';
function row(rule_id = readonly, patch: Partial<RuleSummary> = {}): RuleSummary {
  return {
    rule_id,
    phase: 'completed',
    files: 2,
    bytes: '1000',
    skipped: 1,
    blocked: false,
    note: null,
    scan_failed: false,
    scan_issue: null,
    availability: null,
    reasons: [{ code: 'recent', count: 1 }],
    coverage: { complete: true, issues: [] },
    observation: {
      retained_files: 1,
      retained_bytes: '200',
      recent_files: 1,
      recent_bytes: '200',
      application: null,
      reasons: [{ code: 'recent', files: 1, bytes: '200' }],
    },
    ...patch,
  };
}
function decision(rows: RuleSummary[], selected = new Set<string>(), extra = {}) {
  return cleanupDecision({
    rules,
    rows,
    selected,
    excluded: new Map(),
    sessions: [{ scope: 'user', id: 's' }],
    reports: [],
    canPlan: true,
    needsRescan: false,
    uncertain: false,
    stopping: false,
    active: null,
    ...extra,
  });
}
it.each([readonly, allowed])('separates scan states and permission for %s', (id) => {
  const cases = [
    [{ phase: 'pending' }, 'unvisited', null],
    [{ phase: 'pending' }, 'pending', 'scanning'],
    [{ phase: 'scanning' }, 'scanning', 'scanning'],
    [{ phase: 'scanning' }, 'interrupted', null],
    [{ phase: 'completed' }, 'completed', null],
    [{ phase: 'partial' }, 'partial', null],
    [{ phase: 'failed', scan_failed: true }, 'failed', null],
    [{ phase: 'interrupted' }, 'interrupted', null],
    [{ stale: true }, 'stale', null],
  ] as const;
  for (const [patch, state, active] of cases) {
    const d = decision([row(id, patch)], new Set([id]), { active });
    const item = d.items.find((i) => i.rule.id === id)!;
    expect(item.scan?.state).toBe(state);
    if (id === readonly || !['completed', 'partial'].includes(state)) {
      expect(d.totals.bytes).toBe(0n);
      expect(isSelectableCleanup(item)).toBe(false);
    }
    if (id === readonly) expect(item.capability).toBe('readonly');
    if (['failed', 'interrupted', 'unvisited'].includes(state))
      expect(ruleFollowup(item).label).toBe('需重新扫描');
  }
  expect(decision([]).items.find((i) => i.rule.id === id)?.scan?.state).toBe('not_scanned');
  expect(
    decision([], new Set(), { sessions: [] }).items.find((i) => i.rule.id === id)?.scan?.state,
  ).toBe('unscanned');
});
it('requires a complete empty read before declaring a standard location missing', () => {
  const empty = row(readonly, {
    files: 0,
    bytes: '0',
    skipped: 0,
    reasons: [],
    scan_issue: 'location_missing',
    observation: {
      retained_files: 0,
      retained_bytes: '0',
      recent_files: 0,
      recent_bytes: '0',
      application: null,
    },
  });
  expect(assessScan(empty).absent).toBe(true);
  for (const patch of [
    { files: 1, bytes: '1' },
    { observation: row().observation },
    { scan_failed: true },
    { phase: 'interrupted' as const },
    { coverage: { complete: false, issues: [{ code: 'access_denied' as const, count: 1 }] } },
  ])
    expect(assessScan({ ...empty, ...patch }).absent).toBe(false);
  expect(
    decision([{ ...empty, scan_failed: true }]).items.find((i) => i.rule.id === readonly)?.scan
      ?.label,
  ).toBe('读取失败');
});
it('never allocates running scan candidates to retention because selection is temporarily blocked', () => {
  const rows = [row(allowed, { phase: 'scanning', blocked: true })];
  const d = decision(rows, new Set([allowed]), { active: 'scanning' });
  const totals = scanAccountingFor(d.items, rows);
  expect(totals.retained).toBe(200n);
  expect(scanAmount(totals, 'available')).toBe('待确定');
  expect(scanAmount(totals, 'retained')).toBe('待确定');
  expect(scanAmount(totals, 'observed')).toContain('未知大小');
});
it('keeps selection independent and accounts readonly candidates exactly once with ranked reasons', () => {
  const rows = [row(readonly), row(allowed)];
  const a = scanAccountingFor(decision(rows).items, rows);
  const b = scanAccountingFor(decision(rows, new Set([readonly, allowed])).items, rows);
  expect(a).toEqual(b);
  expect(a).toMatchObject({ observed: 2400n, available: 1000n, retained: 1400n, unknown: false });
  expect(a.reasons).toEqual([
    { label: '尚未开放清理', bytes: 1000n, count: 2 },
    { label: '近期修改，按保留期限留下', bytes: 400n, count: 2 },
  ]);
  const old = [row(readonly, { observation: undefined })];
  const unknown = scanAccountingFor(decision(old).items, old);
  expect(unknown.unknown).toBe(true);
  expect(unknown.reasons.find((r) => r.label.includes('近期'))?.bytes).toBeNull();
  const mixed = scanAccountingFor(
    decision([row(readonly), ...old.map((r) => ({ ...r, rule_id: allowed }))]).items,
    [row(readonly), ...old.map((r) => ({ ...r, rule_id: allowed }))],
  );
  expect(mixed.reasons.filter((r) => r.label.includes('近期')).map((r) => r.bytes)).toEqual([
    200n,
    null,
  ]);
});
it('ignores official tools and unscanned catalog entries when measuring completeness', () => {
  const rows = [
    row(allowed),
    row('dev.uv', { phase: 'failed', scan_failed: true, bytes: '999999999' }),
  ];
  const totals = scanAccountingFor(decision(rows).items, rows);
  expect(totals).toMatchObject({
    observed: 1200n,
    available: 1000n,
    retained: 200n,
    incomplete: false,
    unknown: false,
  });
  const noRows = scanAccountingFor(decision([]).items, []);
  for (const field of ['observed', 'retained', 'available'] as const)
    expect(scanAmount(noRows, field)).toBe('待扫描');
});
it('fences stale scope snapshots and preserves user results when system authorization has no session', () => {
  const user: Summary = { session_id: 'u', state: 'ready', rules: [row(allowed)] };
  const system: Summary = { session_id: 's', state: 'ready', rules: [row('windows.system_temp')] };
  const jobs = {
    user: { id: 'u', state: 'ready' } as Job,
    system: { id: 's', state: 'ready' } as Job,
  };
  expect(
    currentScanRows([{ scope: 'user', id: 'u' }], { user, system }, jobs).map((r) => r.rule_id),
  ).toEqual([allowed]);
  expect(currentScanRows([{ scope: 'user', id: 'new' }], { user }, jobs)).toEqual([]);
  expect(
    currentScanRows(
      [
        { scope: 'user', id: 'u' },
        { scope: 'system', id: 'new' },
      ],
      { user, system },
      jobs,
    ).map((r) => r.rule_id),
  ).toEqual([allowed]);
  expect(
    currentScanRows(
      [{ scope: 'user', id: 'u' }],
      { user },
      { user: { ...jobs.user, state: 'scanning' } },
    ),
  ).toEqual([]);
});
it('shows matching retained occupancy on the overview, collapsed category and readonly rule', () => {
  const rows = [row()];
  const d = decision(rows);
  const item = d.items.find((i) => i.rule.id === readonly)!;
  const model = {
    decision: d,
    summaryRows: rows,
    reports: [],
    totals: { byRule: new Map() },
    sessions: [{ scope: 'user', id: 's' }],
    busy: false,
    saving: false,
    sessionKey: 's',
  } as unknown as AppController;
  const view = render(
    <>
      <ScanTotals model={model} />
      <PurposeGroups model={model} items={[item]} />
      <CleanupRuleList model={model} rules={[item.rule]} />
    </>,
  );
  const stats = screen.getByLabelText('扫描容量统计');
  expect(within(stats).getByText('已保留').nextElementSibling).toHaveTextContent('1.2 KB');
  expect(screen.getByRole('button', { name: '查看日志与错误报告未清理项' })).toHaveTextContent(
    '已保留 1.2 KB · 尚未开放清理',
  );
  expect(view.container.querySelector('.rule-amount strong')).toHaveTextContent('已识别 1.2 KB');
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
});
it('labels post-cleanup Weixin data as a scan snapshot without presenting deleted bytes as retained', () => {
  const rows = [row('weixin.logs')];
  const model = {
    decision: decision(rows),
    summaryRows: rows,
    reports: [{}],
    busy: false,
    saving: false,
    totals: { byRule: new Map() },
    sessions: [{ scope: 'user', id: 's' }],
  } as unknown as AppController;
  render(<WeixinCleanup model={model} onAttachments={() => {}} />);
  fireEvent.click(screen.getByRole('button', { name: '微信专项清理' }));
  const totals = screen.getByLabelText('微信缓存与日志容量');
  expect(within(totals).getByText('扫描时缓存与日志占用')).toBeVisible();
  expect(within(totals).queryByText('已保留')).not.toBeInTheDocument();
  expect(within(totals).queryByText('可直接清理')).not.toBeInTheDocument();
});
