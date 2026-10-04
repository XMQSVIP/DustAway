import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { applicationGroups } from '../cleanupPurposes';
import { CleanupRuleList } from '../CleanupRuleList';
import type { AppController } from '../useAppController';
import rules from '../previewRules';

afterEach(cleanup);
it('groups new and existing editor rules under a stable application identity', () => {
  const ids = [
    'dev.vscode_cache',
    'appcache.vscode_vsix',
    'appcache.vscode_code',
    'appcache.trae_cn',
  ];
  const groups = applicationGroups(ids.map((id) => rules.find((r) => r.id === id)!));
  expect(groups.map((g) => g.id)).toEqual(['vscode', 'trae_cn']);
  expect(groups[0].rules.map((r) => r.id)).toEqual(ids.slice(0, 3));
});
it('shows measured recent cache and uncertain ownership without a cleanup checkbox', () => {
  const rule = rules.find((r) => r.id === 'appcache.cherry')!;
  const model = {
    summaryRows: [
      {
        rule_id: rule.id,
        files: 0,
        bytes: '0',
        phase: 'completed',
        reasons: [],
        observation: {
          retained_files: 2,
          retained_bytes: '4096',
          recent_files: 2,
          recent_bytes: '4096',
          application: {
            title: rule.title,
            identity_status: 'unverified_data',
            locations: ['C:\\data\\Cache'],
            inventory_complete: true,
          },
        },
      },
    ],
    decision: {
      items: [{ rule, capability: 'readonly', status: 'readonly', explanation: rule.impact }],
    },
    totals: { byRule: new Map() },
    reports: [],
    busy: false,
    saving: false,
    openDetail: vi.fn(),
  } as unknown as AppController;
  const view = render(<CleanupRuleList model={model} rules={[rule]} />);
  expect(screen.getByText(/可能为残留/)).toBeInTheDocument();
  expect(screen.getByText(/已读保留内容/)).toHaveTextContent('4.0 KB');
  expect(screen.getByText(/未满足最后修改时间条件/)).toBeInTheDocument();
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: /查看/ })).toBeDisabled();
  model.summaryRows[0] = {
    ...model.summaryRows[0],
    observation: null,
    phase: 'failed',
    scan_failed: true,
    coverage: { complete: false, issues: [] },
  };
  view.rerender(<CleanupRuleList model={model} rules={[rule]} />);
  expect(screen.getByText('未取得')).toBeInTheDocument();
  expect(screen.getByText('文件数量未取得')).toBeInTheDocument();
});
