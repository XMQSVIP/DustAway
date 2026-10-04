import { describe, expect, it } from 'vitest';
import { selectedTotals } from '../selection';
import type { ExcludedFile, RuleSummary } from '../types';
describe('selection totals', () => {
  it('counts explicit files only, ignores stale sessions, and never double counts bulk selection', () => {
    const rows = [
      { phase: 'completed', rule_id: 'review', files: 100, bytes: '10000', blocked: false },
    ] as RuleSummary[];
    const included = new Map<string, ExcludedFile>([
      ['one', { scope: 'user', session: 'current', rule_id: 'review', bytes: '100' }],
      ['old', { scope: 'user', session: 'old', rule_id: 'review', bytes: '500' }],
    ]);
    const sessions = [{ scope: 'user' as const, id: 'current' }];
    expect(selectedTotals(rows, new Set(), new Map(), sessions, included)).toMatchObject({
      files: 1,
      bytes: 100n,
    });
    expect(selectedTotals(rows, new Set(['review']), new Map(), sessions, included)).toMatchObject({
      files: 100,
      bytes: 10000n,
    });
    rows[0].blocked = true;
    expect(selectedTotals(rows, new Set(), new Map(), sessions, included).files).toBe(0);
  });
  it('allows verified partial candidates but rejects failed, interrupted and blocked rows', () => {
    const rows = [
      { phase: 'partial', rule_id: 'partial', files: 2, bytes: '123', blocked: false },
      { phase: 'partial', rule_id: 'blocked', files: 2, bytes: '456', blocked: true },
      { phase: 'failed', rule_id: 'failed', files: 2, bytes: '789', blocked: false },
      { phase: 'interrupted', rule_id: 'interrupted', files: 2, bytes: '1011', blocked: false },
    ] as RuleSummary[];
    expect(selectedTotals(rows, new Set(rows.map((r) => r.rule_id)), new Map(), []).bytes).toBe(
      123n,
    );
  });
  it('tracks exclusions by session and rule without rounding large byte counts', () => {
    const rows = [
      { phase: 'completed', rule_id: 'one', files: 2, bytes: '9007199254740994', blocked: false },
      { phase: 'completed', rule_id: 'two', files: 1, bytes: '4096', blocked: false },
      { phase: 'completed', rule_id: 'blocked', files: 1, bytes: '8192', blocked: true },
    ] as RuleSummary[];
    const excluded = new Map<string, ExcludedFile>([
      ['a', { scope: 'user', session: 'current', rule_id: 'one', bytes: '1' }],
      ['b', { scope: 'user', session: 'old', rule_id: 'one', bytes: '123' }],
      ['c', { scope: 'user', session: 'current', rule_id: 'two', bytes: '4096' }],
    ]);
    const total = selectedTotals(rows, new Set(['one', 'blocked']), excluded, [
      { scope: 'user', id: 'current' },
    ]);
    expect(total.files).toBe(1);
    expect(total.bytes).toBe(9007199254740993n);
    expect(total.byRule.has('two')).toBe(false);
  });
});
