import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it } from 'vitest';
import { CoverageNotice } from '../CoverageNotice';
import { CleanupGuidance } from '../CleanupGuidance';
import { cleanupDecision } from '../cleanupDecision';
import rules from '../previewRules';
import type { RuleSummary, Report } from '../types';
afterEach(cleanup);
const rule = rules.find((r) => r.id === 'windows.user_temp')!;
const row = (patch: Partial<RuleSummary> = {}): RuleSummary => ({
  rule_id: rule.id,
  phase: 'completed',
  files: 3,
  bytes: '3072',
  skipped: 0,
  blocked: false,
  note: null,
  scan_failed: false,
  scan_issue: null,
  availability: null,
  reasons: [],
  coverage: { complete: true, issues: [] },
  ...patch,
});
const input = () => ({
  rules: [rule],
  rows: [row()],
  selected: new Set([rule.id]),
  excluded: new Map(),
  sessions: [{ scope: 'user' as const, id: 'scan' }],
  reports: [] as Report[],
  canPlan: true,
  needsRescan: false,
  uncertain: false,
  active: null,
  stopping: false,
});
it('offers application recheck rather than task reconciliation for unknown process state', () => {
  const decision = cleanupDecision({
    ...input(),
    rows: [row({ blocked: true, availability: 'process_unknown' })],
  });
  expect(decision.action).toBe('refresh_apps');
  expect(decision.canPrepare).toBe(false);
});
it('keeps validation pending explicit after an app closes', () => {
  const decision = cleanupDecision({
    ...input(),
    rows: [row({ blocked: true, availability: 'validation_pending' })],
  });
  render(<CleanupGuidance decision={decision} />);
  expect(screen.getByText('此类内容尚未开放清理，已保留。')).toBeVisible();
  expect(decision.action).toBe('analysis');
  expect(decision.canPrepare).toBe(false);
});
it.each(['partial', 'failed', 'interrupted'] as const)(
  'never calls %s zero candidates a complete empty scan',
  (phase) => {
    const decision = cleanupDecision({
      ...input(),
      rows: [
        row({
          phase,
          files: 0,
          bytes: '0',
          coverage: { complete: false, issues: [{ code: 'access_denied', count: 1 }] },
        }),
      ],
    });
    render(<CleanupGuidance decision={decision} />);
    expect(screen.getByText(/仍有位置未检查/)).toBeVisible();
    expect(decision.action).toBe('scan');
  },
);
it('explains missing historical coverage and counts unread events without inventing file totals', async () => {
  const { rerender } = render(<CoverageNotice />);
  expect(screen.getByText(/旧记录未提供扫描完整程度/)).toBeVisible();
  rerender(
    <CoverageNotice
      coverage={{ complete: false, issues: [{ code: 'access_denied', count: 2 }] }}
    />,
  );
  await userEvent.setup().click(screen.getByText(/读取问题 · 2 次/));
  expect(screen.getByText(/次数不代表未读取的文件数量/)).toBeVisible();
  rerender(<CoverageNotice coverage={{ complete: true, issues: [] }} />);
  expect(screen.queryByText(/扫描结果不完整/)).not.toBeInTheDocument();
});
