import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { ScanObservations } from '../ScanObservations';
import { ScanTotals, scanAccounting } from '../ScanTotals';
import { cleanupDecision } from '../cleanupDecision';
import rules from '../previewRules';
import type { AppController } from '../useAppController';
import type { RuleSummary } from '../types';
const mock = vi.hoisted(() => ({ observations: vi.fn() }));
vi.mock('../api', () => ({ desktop: true, api: mock }));
afterEach(cleanup);
function model(patch: Partial<RuleSummary> = {}) {
  const rows: RuleSummary[] = [
    {
      rule_id: 'windows.user_temp',
      phase: 'completed',
      files: 269,
      bytes: '40996396',
      skipped: 2,
      blocked: false,
      note: null,
      scan_failed: false,
      scan_issue: null,
      availability: null,
      reasons: [{ code: 'recent', count: 2 }],
      coverage: { complete: true, issues: [] },
      observation: {
        retained_files: 2,
        retained_bytes: '1000000000',
        recent_files: 2,
        recent_bytes: '1000000000',
        application: null,
      },
      ...patch,
    },
  ];
  return {
    summaryRows: rows,
    decision: cleanupDecision({
      rules,
      rows,
      selected: new Set(),
      excluded: new Map(),
      sessions: [{ scope: 'user', id: 'scan' }],
      reports: [],
      canPlan: true,
      needsRescan: false,
      uncertain: false,
      stopping: false,
      active: null,
    }),
  } as AppController;
}
it('counts observed and retained bytes independently of selection and marks unknown old data', () => {
  const value = model();
  expect(scanAccounting(value)).toMatchObject({
    observed: 1040996396n,
    available: 40996396n,
    retained: 1000000000n,
    unknown: false,
  });
  render(
    <ScanTotals
      model={model({
        observation: undefined,
        files: 0,
        bytes: '0',
        phase: 'partial',
        coverage: { complete: false, issues: [{ code: 'access_denied', count: 1 }] },
      })}
    />,
  );
  expect(screen.getAllByText('未知')).toHaveLength(2);
  expect(screen.getByText(/部分位置未读取/)).toBeInTheDocument();
});
it('pages read-only retained metadata without offering selection or reusing a stale session', async () => {
  mock.observations.mockImplementation(async (_scope, session, _rule, offset) => ({
    total: 51,
    offset,
    items: [
      {
        path: `C:\\Temp\\${session}-${offset}.zip`,
        bytes: '1024',
        modified_unix: 100,
        reason: 'recent',
      },
    ],
  }));
  const view = render(<ScanObservations scope="user" session="one" rule="windows.user_temp" />);
  await screen.findByText('C:\\Temp\\one-0.zip');
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
  await userEvent.setup().click(screen.getByRole('button', { name: '下一页' }));
  await screen.findByText('C:\\Temp\\one-50.zip');
  expect(mock.observations).toHaveBeenLastCalledWith('user', 'one', 'windows.user_temp', 50);
  view.rerender(<ScanObservations key="two" scope="user" session="two" rule="windows.user_temp" />);
  await waitFor(() =>
    expect(mock.observations).toHaveBeenLastCalledWith('user', 'two', 'windows.user_temp', 0),
  );
  expect(screen.queryByText('C:\\Temp\\one-50.zip')).not.toBeInTheDocument();
});
