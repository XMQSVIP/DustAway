import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { applicationGroups, cleanupPurpose } from '../cleanupPurposes';
import { cleanupDecision } from '../cleanupDecision';
import { scanAccounting } from '../ScanTotals';
import type { RuleSummary } from '../types';
import appCaches from '../../rules/application-cache.json';
import { CleanupEvidence } from '../CleanupEvidence';
import { CleanupRuleList } from '../CleanupRuleList';
import type { AppController } from '../useAppController';
import rules from '../previewRules';

afterEach(cleanup);
it('keeps the eight new observations readonly beside a selected temporary-file category', () => {
  const ids = [
    'codebuddy_extension_logs',
    'workbuddy_extension_logs',
    'todesk_logs',
    'todesk_web',
    'dingtalk_logs',
    'webtoapi_web',
    'webtoapi_logs',
    'dbx_web',
  ].map((id) => 'appcache.' + id);
  const chosen = rules.filter((r) => ids.includes(r.id) || r.id === 'windows.user_temp');
  expect(chosen).toHaveLength(9);
  const rows: RuleSummary[] = chosen.map((rule) => ({
    rule_id: rule.id,
    files: 1,
    bytes: '1024',
    skipped: 0,
    reasons: [],
    blocked: false,
    phase: 'completed',
    scan_failed: false,
    scan_issue: null,
    note: null,
    availability: rule.id === 'windows.user_temp' ? null : 'validation_pending',
    coverage: { complete: true, issues: [] },
    observation: {
      retained_files: 0,
      retained_bytes: '0',
      recent_files: 0,
      recent_bytes: '0',
      reasons: [],
      application: null,
    },
  }));
  const decide = (selected: Set<string>) =>
    cleanupDecision({
      rules: chosen,
      rows,
      selected,
      excluded: new Map(),
      sessions: [{ scope: 'user', id: 'scope-024' }],
      reports: [],
      canPlan: true,
      needsRescan: false,
      uncertain: false,
      stopping: false,
      active: null,
    });
  const normal = decide(new Set(['windows.user_temp']));
  const forged = decide(new Set(chosen.map((r) => r.id)));
  expect(scanAccounting({ decision: normal, summaryRows: rows })).toMatchObject({
    observed: 9216n,
    retained: 8192n,
    available: 1024n,
  });
  expect(scanAccounting({ decision: forged, summaryRows: rows })).toEqual(
    scanAccounting({ decision: normal, summaryRows: rows }),
  );
  expect(
    forged.items
      .filter((i) => ids.includes(i.rule.id))
      .every((i) => i.capability === 'readonly' && i.bytes === 0n && i.files === 0),
  ).toBe(true);
  rows[0] = { ...rows[0], phase: 'partial', coverage: { complete: false, issues: [] } };
  const partial = decide(new Set(['windows.user_temp']));
  expect(scanAccounting({ decision: partial, summaryRows: rows }).unknown).toBe(true);
});
it('keeps all observed scopes out of selection and counts their occupied bytes as retained', () => {
  const added = rules.filter((r) =>
    appCaches.some((s) => s.id === r.id && s.kind === 'readonly_observed'),
  );
  expect(added).toHaveLength(28);
  const rows: RuleSummary[] = added.map((rule) => ({
    rule_id: rule.id,
    files: 2,
    bytes: '1000',
    skipped: 1,
    reasons: [],
    blocked: true,
    phase: 'completed',
    scan_failed: false,
    scan_issue: null,
    note: null,
    availability: 'validation_pending',
    coverage: { complete: true, issues: [] },
    observation: {
      retained_files: 1,
      retained_bytes: '200',
      recent_files: 0,
      recent_bytes: '0',
      reasons: [],
      application: null,
    },
  }));
  const decide = (selected: Set<string>) =>
    cleanupDecision({
      rules: added,
      rows,
      selected,
      excluded: new Map(),
      sessions: [{ scope: 'user', id: 'current' }],
      reports: [],
      canPlan: true,
      needsRescan: false,
      uncertain: false,
      stopping: false,
      active: null,
    });
  const unselected = decide(new Set());
  const selected = decide(new Set(added.map((r) => r.id)));
  expect(
    selected.items.every((i) => i.capability === 'readonly' && i.files === 0 && i.bytes === 0n),
  ).toBe(true);
  expect(scanAccounting({ decision: unselected, summaryRows: rows })).toEqual(
    scanAccounting({ decision: selected, summaryRows: rows }),
  );
  expect(scanAccounting({ decision: selected, summaryRows: rows })).toMatchObject({
    observed: 33600n,
    retained: 33600n,
    available: 0n,
    unknown: false,
  });
  for (const r of added.filter((r) => r.data_kind === 'diagnostics'))
    expect(cleanupPurpose(r.id)?.id).toBe('diagnostics');
  expect(applicationGroups(added).map((g) => g.id)).toEqual([
    'shadowbot',
    'codex',
    'claude',
    'docker',
    'reqable',
    'trae',
    'trae_cn',
    'kiro',
    'workbuddy',
    'doubao',
    'gameviewer',
    'ccswitch',
    'ima',
    'easyspider',
    'codebuddy_extension',
    'workbuddy_extension',
    'todesk',
    'dingtalk',
    'webtoapi',
    'dbx',
  ]);
});
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
  expect(screen.getByText('发现相关数据，安装身份未确认')).toBeInTheDocument();
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
  expect(screen.getByText('未知大小')).toBeInTheDocument();
  expect(screen.getByText('文件数量未取得')).toBeInTheDocument();
});

it('labels observed evidence by its actual source and preserves logs without official claims', () => {
  const view = render(<CleanupEvidence ruleId="appcache.ccswitch_web" />);
  expect(screen.getByRole('link', { name: /WebView2 用户目录说明/ })).toBeInTheDocument();
  expect(screen.queryByText('查看官方依据')).not.toBeInTheDocument();
  view.rerender(<CleanupEvidence ruleId="appcache.codex_logs" />);
  expect(screen.getByText(/尚无足够的清理依据/)).toBeInTheDocument();
  expect(screen.getAllByText(/固定目录来自本机观察/).length).toBeGreaterThan(0);
});
