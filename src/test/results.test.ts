import { expect, it } from 'vitest';
import {
  consistentReport,
  expectedRetention,
  priorityResultStatus,
  resultTargets,
  candidateNextStep,
  cleanupResultSummary,
  resultTitle,
  formatRecordedBytes,
  formatResultCount,
} from '../cleanupResults';
import { cleanupDecision } from '../cleanupDecision';
import type { Candidate, ReasonCode, Report, RuleOutcome } from '../types';
import rules from '../previewRules';

const category = (patch: Partial<RuleOutcome> = {}): RuleOutcome => ({
  rule_id: 'windows.user_temp',
  total: 3,
  deleted: 1,
  skipped: 2,
  failed: 0,
  unknown: 0,
  deleted_bytes: '1',
  reasons: [{ code: 'protected', count: 2 }],
  retained: [],
  ...patch,
});
const report = (patch: Partial<Report> = {}): Report => ({
  id: 'plan',
  scope: 'user',
  status: 'finished',
  started_at: 1,
  finished_at: 2,
  total: 3,
  deleted: 1,
  skipped: 2,
  failed: 0,
  unknown: 0,
  candidate_bytes: '3',
  deleted_bytes: '1',
  free_space_change: null,
  categories: [category()],
  ...patch,
});
function decision(value: Report) {
  return cleanupDecision({
    rules,
    rows: [],
    selected: new Set(),
    excluded: new Map(),
    sessions: [],
    reports: [value],
    canPlan: false,
    needsRescan: false,
    uncertain: false,
    active: null,
    stopping: false,
  });
}
it('accounts for mixed sources and never hides changed manual files behind rule retention', () => {
  const value = report({
    total: 4,
    skipped: 3,
    manual: category({
      rule_id: '',
      total: 1,
      deleted: 0,
      skipped: 1,
      reasons: [{ code: 'changed', count: 1 }],
    }),
  });
  expect(consistentReport(value)).toBe(true);
  expect(expectedRetention(value)).toBe(false);
  expect(decision(value).action).toBe('finish');
  value.manual!.total = 2;
  expect(consistentReport(value)).toBe(false);
});
it('keeps directory uncertainty separate from file totals and routes to the report', () => {
  const value = report({
    total: 0,
    deleted: 0,
    skipped: 0,
    categories: [],
    directories: { total: 1, deleted: 0, skipped: 0, failed: 0, unknown: 1 },
  });
  expect(consistentReport(value)).toBe(true);
  expect(decision(value).action).toBe('review_unknown');
  value.directories!.unknown = 0;
  value.directories!.failed = 1;
  expect(decision(value).action).toBe('finish');
});
it.each(['recent', 'protected', 'excluded', 'link_or_cloud'] as ReasonCode[])(
  'allows space review only for fully accounted %s retention',
  (code) => {
    const value = report({
      categories: [
        category({
          reasons: [{ code, count: 2 }],
          retained: [{ code: 'access_denied', count: 99 }],
        }),
      ],
    });
    expect(expectedRetention(value)).toBe(true);
    expect(decision(value).action).toBe('finish');
    expect(decision(value).message).toContain('清理已完成');
    expect(priorityResultStatus([value])).toBeNull();
  },
);
it.each([
  'in_use',
  'changed',
  'access_denied',
  'unverifiable',
  'unknown',
  'process_unknown',
] as ReasonCode[])('keeps %s retention actionable', (code) => {
  const value = report({ categories: [category({ reasons: [{ code, count: 2 }] })] });
  expect(expectedRetention(value)).toBe(false);
  expect(decision(value).actionLabel).toBe('完成');
});
it.each([
  report({ categories: [] }),
  report({ categories: [category({ reasons: [] })] }),
  report({ categories: [category({ reasons: [{ code: 'protected', count: 1 }] })] }),
  report({ categories: [category({ total: 4 })] }),
  report({ categories: [category({ reasons: [], retained: [{ code: 'protected', count: 2 }] })] }),
  report({
    categories: [
      category({
        reasons: [
          { code: 'protected', count: 1 },
          { code: 'protected', count: 1 },
        ],
      }),
    ],
  }),
  report({ status: 'cancelled' }),
  report({ status: 'interrupted' }),
  report({ total: 4 }),
  report({ deleted: 9 }),
  report({ skipped: -1 }),
])('does not treat incomplete or inconsistent report %# as normal retention', (value) => {
  expect(expectedRetention(value)).toBe(false);
  expect(decision(value).action).not.toBe('refresh_space');
});
it('prioritizes unknown, failed, unprocessed, then non-normal retention', () => {
  const value = report({ total: 8, deleted: 1, skipped: 2, unknown: 1, failed: 1, categories: [] });
  expect(priorityResultStatus([value])).toBe('unknown');
  expect(priorityResultStatus([{ ...value, unknown: 0 }])).toBe('failed');
  expect(priorityResultStatus([{ ...value, unknown: 0, failed: 0 }])).toBe('candidate');
  expect(priorityResultStatus([report({ categories: [] })])).toBe('skipped');
  expect(
    decision(report({ status: 'cancelled', total: 0, deleted: 0, skipped: 0, categories: [] }))
      .actionLabel,
  ).toBe('完成');
});
it('rejects duplicate categories even if aggregate counts happen to add up', () => {
  expect(
    consistentReport(
      report({ total: 6, deleted: 2, skipped: 4, categories: [category(), category()] }),
    ),
  ).toBe(false);
});
it('binds review targets to current snapshot identity and sorts by count', () => {
  const value = report({
    categories: [category(), category({ rule_id: 'edge.http', total: 4, skipped: 3 })],
  });
  const context = {
    jobs: {
      user: {
        id: 'scan',
        state: 'finished',
        current_rule: '',
        visited: 7,
        completed: 7,
        total: 7,
        error: null,
      } as const,
    },
    summaries: {
      user: {
        session_id: 'scan',
        state: 'finished' as const,
        rules: [{ rule_id: 'windows.user_temp' }, { rule_id: 'edge.http' }] as any,
      },
    },
  };
  const targets = resultTargets([value], rules, [{ scope: 'user', id: 'scan' }], context).filter(
    (t) => t.status === 'skipped',
  );
  expect(targets.map((t) => t.ruleId)).toEqual(['edge.http', 'windows.user_temp']);
  expect(targets.every((t) => t.session === 'scan' && t.reportId === 'plan')).toBe(true);
  expect(
    resultTargets([value], rules, [{ scope: 'user', id: 'new-scan' }], context).every(
      (t) => t.session === null,
    ),
  ).toBe(true);
  expect(resultTargets([value], rules, [], context).every((t) => t.session === null)).toBe(true);
});
it.each([
  ['unknown', 'protected', '不会自动重试'],
  ['candidate', null, '本次未执行'],
  ['failed', 'delete_failed', '原因可展开'],
  ['skipped', 'in_use', '文件正在使用'],
  ['skipped', 'changed', '重新扫描以核对最新文件'],
  ['skipped', 'excluded', '排除设置'],
  ['skipped', 'access_denied', '当前权限无法处理'],
] as const)('provides next step for %s/%s', (status, reason_code, expected) => {
  expect(candidateNextStep({ status, reason_code } as Candidate)).toContain(expected);
});

it.each([
  [{ total: 1, deleted: 1, skipped: 0, failed: 0, unknown: 0 }, 'complete'],
  [{ total: 1, deleted: 0, skipped: 0, failed: 0, unknown: 0 }, 'ended'],
  [{ total: 1, deleted: 0, skipped: 0, failed: 1, unknown: 0 }, 'ended'],
  [{ total: 1, deleted: 0, skipped: 0, failed: 0, unknown: 1 }, 'unconfirmed'],
  [{ total: 1, deleted: 2, skipped: 0, failed: 0, unknown: 0 }, 'unconfirmed'],
  [{ total: 1, deleted: -1, skipped: 0, failed: 0, unknown: 0 }, 'unconfirmed'],
] as const)(
  'shares directory result priority between receipt, summary and history: %j',
  (directories, state) => {
    const value = report({ total: 0, deleted: 0, skipped: 0, categories: [], directories });
    const result = cleanupResultSummary([value]);
    expect(result.state).toBe(state);
    expect(decision(value).result).toEqual(result);
    expect(resultTitle(value)).toBe(result.title);
    expect(result.files.total).toBe(0);
    expect(result.directories.total).toBe(1);
  },
);
it('preserves inconsistent counts, displays invalid fields as unavailable and accepts legacy omissions', () => {
  const value = report({ total: 1, deleted: 2, skipped: 0, categories: [] });
  expect(cleanupResultSummary([value])).toMatchObject({
    state: 'unconfirmed',
    deleted: 2,
    total: 1,
  });
  expect(value.deleted).toBe(2);
  expect(formatResultCount(cleanupResultSummary([value]).unprocessed)).toBe('统计不可用');
  expect(cleanupResultSummary([{ ...value, deleted_bytes: 'broken' }]).bytes).toBeNull();
  for (const bytes of ['bad', '-1', '18446744073709551616'])
    expect(formatRecordedBytes(bytes)).toBe('统计不可用');
  for (const count of [-1, NaN, Infinity, 0.5, Number.MAX_SAFE_INTEGER + 1])
    expect(formatResultCount(count)).toBe('统计不可用');
  expect(
    cleanupResultSummary([
      report({ total: 1, deleted: 1, skipped: 0, categories: undefined, directories: undefined }),
    ]).state,
  ).toBe('complete');
  expect(decision(value).action).toBe('review_unknown');
});
