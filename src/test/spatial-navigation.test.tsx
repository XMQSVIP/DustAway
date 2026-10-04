import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { CleanupGroups } from '../CleanupGroups';
import { cleanupDecision } from '../cleanupDecision';
import rules from '../previewRules';
import type { AppController } from '../useAppController';
afterEach(cleanup);
it('opens the requested scope on every handoff without granting cleanup selection', async () => {
  const decision = cleanupDecision({
    rules,
    rows: [],
    selected: new Set(),
    excluded: new Map(),
    sessions: [],
    reports: [],
    canPlan: false,
    needsRescan: false,
    uncertain: false,
    active: null,
    stopping: false,
  });
  const model = {
    decision,
    boot: { rules, settings: { excluded_rules: [] } },
    summaryRows: [],
    reports: [],
    selected: new Set(),
    totals: decision.totals,
    applicationInfo: { applications: [], loading: false, error: null },
    busy: false,
    toggle: vi.fn(),
    selectRecommended: vi.fn(),
  } as unknown as AppController;
  const props = { model, items: decision.items, catalog: true };
  const { rerender } = render(<CleanupGroups {...props} />);
  rerender(<CleanupGroups {...props} inspectedRules={['edge.http']} />);
  expect(screen.getByRole('button', { name: 'Microsoft Edge' })).toHaveAttribute(
    'aria-expanded',
    'true',
  );
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Microsoft Edge' }));
  expect(screen.getByRole('button', { name: 'Microsoft Edge' })).toHaveAttribute(
    'aria-expanded',
    'false',
  );
  rerender(<CleanupGroups {...props} inspectedRules={['edge.http']} />);
  expect(screen.getByRole('button', { name: 'Microsoft Edge' })).toHaveAttribute(
    'aria-expanded',
    'true',
  );
  rerender(<CleanupGroups {...props} inspectedRules={['wps.logs']} />);
  expect(screen.getByRole('button', { name: 'WPS' })).toHaveAttribute('aria-expanded', 'true');
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
  expect(model.toggle).not.toHaveBeenCalled();
  expect(model.selectRecommended).not.toHaveBeenCalled();
});
