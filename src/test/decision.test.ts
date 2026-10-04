import { cleanupPresentation } from '../cleanupPresentation';
import type { AppController } from '../useAppController';
import { expect, it } from 'vitest';
import { cleanupDecision } from '../cleanupDecision';
import rules from '../previewRules';
import { applicationFixtures } from './applicationFixtures';
import type { RuleSummary, Report, ExcludedFile } from '../types';

function row(rule_id: string, patch: Partial<RuleSummary> = {}): RuleSummary {
  return {
    rule_id,
    files: 2,
    bytes: '9007199254740994',
    phase: 'completed',
    skipped: 0,
    blocked: false,
    note: null,
    scan_failed: false,
    scan_issue: null,
    availability: null,
    reasons: [],
    coverage: { complete: true, issues: [] },
    ...patch,
  };
}
const input = () => ({
  rules,
  applications: applicationFixtures,
  rows: [row('windows.user_temp')],
  selected: new Set(['windows.user_temp']),
  excluded: new Map<string, ExcludedFile>(),
  sessions: [{ scope: 'user' as const, id: 'current' }],
  reports: [] as Report[],
  canPlan: true,
  needsRescan: false,
  uncertain: false,
  active: null,
  stopping: false,
});
const report = (patch: Partial<Report> = {}): Report => ({
  id: 'report',
  scope: 'user',
  started_at: 1,
  finished_at: 2,
  status: 'finished',
  total: 2,
  deleted: 2,
  skipped: 0,
  failed: 0,
  unknown: 0,
  candidate_bytes: '2048',
  deleted_bytes: '2048',
  free_space_change: null,
  categories: [],
  ...patch,
});

it('keeps mixed states, including two states in one app, and counts only eligible selection', () => {
  const rows = [
    row('windows.user_temp'),
    row('edge.http'),
    row('edge.gpu', { blocked: true, availability: 'browser_running' }),
    row('weixin.logs', { blocked: true, availability: 'validation_pending' }),
    row('chrome.http', { blocked: true, phase: 'failed', scan_failed: true }),
  ];
  const decision = cleanupDecision({
    ...input(),
    rows,
    selected: new Set(rows.map((r) => r.rule_id)),
  });
  expect(decision.chosen.map((r) => r.status)).toEqual(
    expect.arrayContaining(['ready', 'running', 'readonly', 'incomplete']),
  );
  expect(decision.totals.files).toBe(4);
  expect(decision.totals.bytes).toBe(18014398509481988n);
  expect(decision.action).toBe('prepare');
});
it('does not let an unselected blocker take over next steps', () => {
  const decision = cleanupDecision({
    ...input(),
    rows: [
      row('windows.user_temp', { files: 0, bytes: '0' }),
      row('weixin.logs', { blocked: true, availability: 'validation_pending' }),
    ],
  });
  expect(decision.message).toContain('所选范围内没有符合规则');
  expect(decision.chosen).toHaveLength(1);
});
it('subtracts only current-session exclusions using exact bytes', () => {
  const value = input();
  value.excluded.set('old', {
    scope: 'user',
    session: 'old',
    rule_id: 'windows.user_temp',
    bytes: '99',
  });
  value.excluded.set('current', {
    scope: 'user',
    session: 'current',
    rule_id: 'windows.user_temp',
    bytes: '9007199254740993',
  });
  const decision = cleanupDecision(value);
  expect(decision.totals.files).toBe(1);
  expect(decision.totals.bytes).toBe(1n);
});
it('distinguishes deselected files from a complete zero result', () => {
  const value = input();
  value.excluded.set('a', {
    scope: 'user',
    session: 'current',
    rule_id: 'windows.user_temp',
    bytes: '9007199254740994',
  });
  value.excluded.set('b', {
    scope: 'user',
    session: 'current',
    rule_id: 'windows.user_temp',
    bytes: '0',
  });
  expect(cleanupDecision(value).action).toBe('choose');
  expect(cleanupDecision(value).chosen[0].status).toBe('deselected');
  expect(
    cleanupDecision({ ...input(), rows: [row('windows.user_temp', { files: 0, bytes: '0' })] })
      .chosen[0].status,
  ).toBe('empty');
});
it('allows verified partial candidates but rejects interrupted or invalidated sessions', () => {
  expect(
    cleanupDecision({
      ...input(),
      rows: [
        row('windows.user_temp', { phase: 'partial', coverage: { complete: false, issues: [] } }),
      ],
    }).canPrepare,
  ).toBe(true);
  expect(
    cleanupDecision({
      ...input(),
      rows: [row('windows.user_temp', { phase: 'interrupted', blocked: true })],
      canPlan: false,
    }).canPrepare,
  ).toBe(false);
  const stale = cleanupDecision({ ...input(), needsRescan: true, canPlan: false });
  expect(stale.action).toBe('scan');
  expect(stale.canPrepare).toBe(false);
});
it('requires scanning newly selected categories instead of assuming zero candidates', () => {
  const decision = cleanupDecision({ ...input(), rows: [], selected: new Set(['edge.http']) });
  expect(decision.chosen[0].status).toBe('needs_scan');
  expect(decision.action).toBe('scan');
});
it.each([
  [{ unknown: 1, deleted: 1 }, 'review_unknown'],
  [{ failed: 1, deleted: 1 }, 'finish'],
  [{ skipped: 1, deleted: 1 }, 'finish'],
  [{ status: 'cancelled', deleted: 1 }, 'finish'],
  [{}, 'finish'],
] as const)('prioritizes report outcome %j without replaying a plan', (patch, action) => {
  const decision = cleanupDecision({ ...input(), reports: [report(patch)] });
  expect(decision.action).toBe(action);
  expect(decision.canPrepare).toBe(false);
});
it('reconciles uncertain requests ahead of reports and offers stop for running work', () => {
  expect(cleanupDecision({ ...input(), uncertain: true, reports: [report()] }).action).toBe(
    'verify',
  );
  expect(cleanupDecision({ ...input(), active: 'cleaning' }).action).toBe('stop');
});

it('does not call a stopped empty plan fully processed', () => {
  const decision = cleanupDecision({
    ...input(),
    reports: [report({ status: 'cancelled', total: 0, deleted: 0 })],
  });
  expect(decision.result.stopped).toBe(true);
  expect(decision.action).toBe('finish');
  expect(decision.message).toContain('清理已停止');
});

it.each(rules.filter((r) => r.app_id))(
  'keeps $id readonly with all runtime blockers and excludes exact bytes',
  (rule) => {
    for (const availability of [
      'application_running',
      'unsupported_version',
      'application_unverified',
      'process_unknown',
      null,
    ] as const) {
      const result = cleanupDecision({
        ...input(),
        rows: [row(rule.id, { availability })],
        selected: new Set([rule.id]),
      });
      expect(result.chosen[0].status).toBe('readonly');
      expect(result.chosen[0].condition.length).toBeGreaterThan(0);
      expect(result.totals.files).toBe(0);
      expect(result.totals.bytes).toBe(0n);
      expect(result.action).not.toBe('refresh_apps');
      expect(result.canPrepare).toBe(false);
    }
  },
);

it('treats missing per-rule capabilities as unknown, never using the legacy aggregate permission', () => {
  for (const applications of [
    [],
    applicationFixtures.map((a) => ({ ...a, rules: undefined, cleanup_validated: true })),
  ]) {
    const result = cleanupDecision({
      ...input(),
      applications,
      rows: [row('weixin.logs'), row('windows.user_temp')],
      selected: new Set(['weixin.logs', 'windows.user_temp']),
    });
    expect(result.items.find((r) => r.rule.id === 'weixin.logs')?.status).toBe(
      'capability_unknown',
    );
    expect(result.totals.files).toBe(2);
    expect(result.action).toBe('prepare');
  }
});

it('supports one approved rule without approving siblings or reviving an invalid session', () => {
  const applications = structuredClone(applicationFixtures);
  const approved = applications[0].rules!.find((rule) => rule.rule_id === 'weixin.logs')!;
  approved.cleanup_validated = true;
  approved.cleanup_versions = [applications[0].version!];
  approved.cleanup_block = null;
  const chosen = new Set(['weixin.logs', 'weixin.dumps']);
  const value = {
    ...input(),
    applications,
    selected: chosen,
    rows: [row('weixin.logs'), row('weixin.dumps')],
  };
  expect(cleanupDecision(value).chosen.map((r) => r.status)).toEqual(['ready', 'readonly']);
  expect(cleanupDecision(value).totals.files).toBe(2);
  expect(cleanupDecision({ ...value, canPlan: false }).canPrepare).toBe(false);
  expect(cleanupDecision({ ...value, needsRescan: true }).canPrepare).toBe(false);
  expect(cleanupDecision({ ...value, reports: [report()] }).canPrepare).toBe(false);
  applications[0].running_components = ['Weixin.exe'];
  applications[0].status = 'application_running';
  const running = cleanupDecision(value);
  expect(running.chosen.map((r) => r.status)).toEqual(['running', 'readonly']);
  expect(running.totals.files).toBe(0);
  expect(running.action).toBe('refresh_apps');
});

it('keeps uncertain and stopping stages ahead of a retained success report', () => {
  const decision = cleanupDecision({ ...input(), reports: [report()] });
  const model = {
    decision,
    reports: [report()],
    summaryRows: [],
    scanned: true,
    currentJob: null,
  } as unknown as AppController;
  expect(cleanupPresentation(model).stage).toBe('complete');
  expect(cleanupPresentation({ ...model, stopping: true }).stage).toBe('stopping');
  expect(cleanupPresentation({ ...model, stopping: true, uncertain: true }).stage).toBe(
    'uncertain',
  );
});
it('keeps a partial cleanup in the attention stage instead of reporting success', () => {
  for (const patch of [
    { failed: 1 },
    { unknown: 1 },
    { status: 'cancelled' as const },
    { total: 3 },
  ]) {
    const reports = [report(patch)];
    const decision = cleanupDecision({ ...input(), reports });
    const model = {
      decision,
      reports,
      summaryRows: [],
      scanned: true,
      currentJob: null,
    } as unknown as AppController;
    expect(cleanupPresentation(model).stage).toBe('attention');
  }
});

it('shows recently modified cache without adding its measured bytes to selection', () => {
  const id = 'appcache.vscode_vsix';
  const decision = cleanupDecision({
    ...input(),
    rows: [
      row(id, {
        files: 0,
        bytes: '0',
        reasons: [{ code: 'recent', count: 2 }],
        observation: {
          retained_files: 2,
          retained_bytes: '1234567',
          recent_files: 2,
          recent_bytes: '1234567',
          application: null,
        },
      }),
    ],
    selected: new Set([id]),
  });
  const item = decision.items.find((r) => r.rule.id === id)!;
  expect(item.recognizedFiles).toBe(2);
  expect(item.recognizedBytes).toBe(1234567n);
  expect(item.files).toBe(0);
  expect(item.bytes).toBe(0n);
  const presentation = cleanupPresentation({
    decision,
    currentJob: { state: 'ready' },
    reports: [],
    scanned: true,
    summaryRows: [],
    hasManual: false,
  } as unknown as AppController);
  expect(presentation.buckets[2].items.map((r) => r.rule.id)).toContain(id);
  expect(presentation.available).toHaveLength(0);
});

it('retains application remnants and pending update contents without granting deletion', () => {
  const ids = ['appcache.cherry', 'appcache.update_cherry'];
  const decision = cleanupDecision({
    ...input(),
    rows: ids.map((id) => row(id)),
    selected: new Set(ids),
  });
  for (const id of ids) {
    const item = decision.items.find((r) => r.rule.id === id)!;
    expect(item.foundFiles).toBe(2);
    expect(item.capability).toBe('readonly');
    expect(item.bytes).toBe(0n);
    expect(item.files).toBe(0);
  }
});
