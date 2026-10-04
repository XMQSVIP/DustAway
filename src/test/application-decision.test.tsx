import { cleanup, render as baseRender, fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { CleanupDecisionPanel } from '../CleanupDecisionPanel';
import { cleanupDecision } from '../cleanupDecision';
import rules from '../previewRules';
import { applicationFixtures } from './applicationFixtures';
import type { AppController } from '../useAppController';
import type { RuleSummary } from '../types';
import { cleanupPurpose, cleanupPurposes, purposeGroups } from '../cleanupPurposes';

afterEach(cleanup);
function render(ui: Parameters<typeof baseRender>[0]) {
  const result = baseRender(ui);
  const summary = screen.queryByText('查看并调整');
  if (summary) fireEvent.click(summary);
  return result;
}

function resultModel(
  rows: RuleSummary[],
  catalog = rules,
  applications = applicationFixtures,
  selected = new Set<string>(),
) {
  return {
    windowsMaintenance: {
      snapshots: {},
      selected: new Set(),
      plans: [],
      error: '',
      busy: false,
      scan: vi.fn(),
      cancel: vi.fn(),
    },
    jobs: {},
    sessions: [{ scope: 'user', id: 'current' }],
    refreshRule: vi.fn(),
    selectionEstimate: { bytes: 0n, files: 0, directories: 0, known: true },
    manualPreview: { data: null, error: '', retry: vi.fn() },
    selectRuleGroup: vi.fn(),
    journey: { locations: [], locationError: '', reloadLocations: vi.fn() },
    scanVolumeSelection: null,
    setScanVolumeSelection: vi.fn(),
    decision: cleanupDecision({
      rules: catalog,
      applications,
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
    }),
    selected,
    totals: { byRule: new Map() },
    busy: false,
    saving: false,
    scanned: true,
    ready: true,
    summaryRows: rows,
    reports: [],
    currentJob: null,
    sessionKey: 'current',
    selectRecommended: vi.fn(),
    openDetail: vi.fn(),
    refreshAvailability: vi.fn(),
  } as unknown as AppController;
}
function resultRow(rule_id: string, patch: Partial<RuleSummary> = {}): RuleSummary {
  return {
    rule_id,
    files: 0,
    bytes: '0',
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

it('orders actionable groups by available bytes and bulk-selects only recommendations', async () => {
  const model = resultModel([
    resultRow('windows.user_temp', { files: 3, bytes: '1000' }),
    resultRow('edge.http', { files: 2, bytes: '9000' }),
    resultRow('dev.npm_logs', { files: 1, bytes: '100000' }),
    resultRow('weixin.logs', { files: 1, bytes: '999999' }),
  ]);
  render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  const group = screen.getByRole('region', { name: '建议清理' });
  const purposes = [...group.querySelectorAll('.purpose-group')].map((e) =>
    e.getAttribute('data-purpose'),
  );
  expect(purposes).toEqual(['browser', 'temporary']);
  await userEvent.setup().click(screen.getByRole('button', { name: '选择建议项' }));
  expect(model.selectRecommended).toHaveBeenCalledWith(
    expect.arrayContaining(['edge.http', 'windows.user_temp']),
    true,
  );
  const ids = vi.mocked(model.selectRecommended).mock.calls[0][0];
  expect(ids).not.toContain('dev.npm_logs');
  expect(ids).not.toContain('weixin.logs');
  await userEvent
    .setup()
    .click(within(group).getByRole('button', { name: '查看浏览器缓存建议项' }));
  expect(model.selectRecommended).toHaveBeenCalledTimes(1);
  expect(model.selectRuleGroup).not.toHaveBeenCalled();
});

it('exposes uv outside retained items without counting its size as selectable', () => {
  const model = resultModel([resultRow('dev.uv', { files: 20, bytes: '1000000' })]);
  model.uv = { task: null, stale: false, error: '' } as AppController['uv'];
  render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  const tools = screen.getByRole('region', { name: '官方工具整理' });
  expect(within(tools).getByRole('button', { name: '整理 uv 闲置缓存' })).toBeVisible();
  expect(screen.queryByText(/本次未清理 ·/)).not.toBeInTheDocument();
  expect(model.decision.totals.bytes).toBe(0n);
  expect(screen.getByRole('heading', { name: '发现可由官方工具整理的缓存' })).toBeVisible();
});

it('hides stale uv size, disables repeat maintenance, and preserves other selections', () => {
  const selected = new Set(['windows.user_temp']);
  const model = resultModel(
    [resultRow('dev.uv', { files: 20, bytes: '1000000' })],
    rules,
    applicationFixtures,
    selected,
  );
  model.uv = { task: null, stale: true, error: '' } as AppController['uv'];
  render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  expect(screen.getByRole('button', { name: '整理 uv 闲置缓存' })).toBeDisabled();
  expect(screen.getByText(/uv 结果待更新/)).toBeVisible();
  expect(screen.queryByText(/扫描发现/)).not.toBeInTheDocument();
  expect(model.selected).toEqual(selected);
});
it('keeps rescan available in compact review while the primary action is docked', async () => {
  const rescan = vi.fn();
  const model = resultModel([resultRow('windows.user_temp', { files: 1, bytes: '100' })]);
  render(
    <CleanupDecisionPanel
      model={model}
      docked
      onReview={vi.fn()}
      primaryAction={<button>清理已选</button>}
      reviewAction={<button onClick={rescan}>重新全面扫描</button>}
    />,
  );
  expect(screen.queryByRole('button', { name: '清理已选' })).not.toBeInTheDocument();
  const button = screen.getByRole('button', { name: '重新全面扫描' });
  expect(button.closest('.scan-scope-summary')).not.toBeNull();
  await userEvent.setup().click(button);
  expect(rescan).toHaveBeenCalledTimes(1);
});
async function openRetained(user: ReturnType<typeof userEvent.setup>, title = '应用缓存') {
  await user.click(screen.getByText(/本次未清理 ·/));
  await user.click(screen.getByRole('button', { name: `查看${title}未清理项` }));
}
it('counts uncleared categories separately from files and explains the next step without selecting', async () => {
  const model = resultModel([
    resultRow('edge.http', {
      files: 80,
      bytes: '800',
      blocked: true,
      availability: 'browser_running',
    }),
    resultRow('weixin.logs', {
      files: 120,
      bytes: '1200',
      blocked: true,
      availability: 'validation_pending',
    }),
    resultRow('windows.user_temp', { files: 0, phase: 'failed', scan_failed: true }),
  ]);
  render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  const user = userEvent.setup();
  await user.click(screen.getByText('本次未清理 · 3 类'));
  expect(screen.getByText(/这里统计的是清理类别，不是文件数量/)).toBeVisible();
  expect(screen.getByText(/需退出应用 1 类/)).toHaveTextContent('尚未开放清理 1 类');
  expect(screen.getByText(/需退出应用 1 类/)).toHaveTextContent('需重新检查 1 类');
  await user.click(screen.getByRole('button', { name: '查看浏览器缓存未清理项' }));
  expect(screen.getByText(/后续处理：保存工作并正常退出/)).toBeVisible();
  await user.click(screen.getByRole('button', { name: '查看日志与错误报告未清理项' }));
  expect(screen.getByText(/重复扫描不会解除此限制/)).toBeVisible();
  expect(model.selected.size).toBe(0);
  expect(model.selectRuleGroup).not.toHaveBeenCalled();
  expect(model.selectRecommended).not.toHaveBeenCalled();
});
it('maps every builtin rule exactly once and keeps the purpose order stable', () => {
  for (const rule of rules) {
    expect(cleanupPurpose(rule.id), rule.id).toBeDefined();
    expect(
      cleanupPurposes.filter((purpose) => (purpose.rules as readonly string[]).includes(rule.id)),
    ).toHaveLength(1);
  }
  const model = resultModel(rules.map((r) => resultRow(r.id, { files: 1, bytes: '1' })));
  expect(purposeGroups([...model.decision.items].reverse()).map((g) => g.id)).toEqual([
    'temporary',
    'browser',
    'application',
    'graphics',
    'developer',
    'diagnostics',
    'updates',
  ]);
});
it('offers developer remnants for explicit selection without counting running tools or readonly files', async () => {
  const developerRules = rules.filter((rule) => rule.id.startsWith('dev.'));
  expect(developerRules).toHaveLength(37);
  expect(
    developerRules.every((rule) => !rule.default_selected && rule.cleanup_policy === 'review'),
  ).toBe(true);
  const rows = [
    resultRow('dev.pip_http', { files: 2, bytes: '2048' }),
    resultRow('dev.npm_cache', {
      files: 1,
      bytes: '4096',
      blocked: true,
      availability: 'application_running',
    }),
    resultRow('baidu_netdisk.http', {
      files: 1,
      bytes: '8192',
      availability: 'validation_pending',
    }),
  ];
  const model = resultModel(rows, rules, []);
  expect(model.decision.totals.bytes).toBe(0n);
  const user = userEvent.setup();
  render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  await user.click(screen.getByText(/可自行选择 ·/));
  const choice = screen.getByRole('checkbox', { name: '选择开发工具缓存可自行选择项' });
  expect(choice).not.toBeChecked();
  await user.click(choice);
  expect(model.selectRuleGroup).toHaveBeenCalledWith(['dev.pip_http'], 'review', true);
  await user.click(screen.getByRole('button', { name: '查看开发工具缓存可自行选择项' }));
  expect(screen.getByText('pip 下载缓存')).toBeVisible();
  expect(screen.queryByText('NuGet 下载缓存')).not.toBeInTheDocument();
  const chosen = resultModel(
    rows,
    rules,
    [],
    new Set(['dev.pip_http', 'dev.npm_cache', 'baidu_netdisk.http']),
  );
  expect(chosen.decision.totals.bytes).toBe(2048n);
  expect(chosen.decision.totals.files).toBe(2);
});
it.each([
  ['feishu.http', 'feishu.code', 'feishu.gpu', 'dingtalk.http', 'dingtalk.code'],
  [
    'baidu_netdisk.http',
    'baidu_netdisk.code',
    'baidu_netdisk.gpu',
    'quark_netdisk.code',
    'quark_netdisk.gpu',
    'thunder.code',
    'thunder.gpu',
  ],
])('keeps readonly applications out of selection even while running: %s', async (...ids) => {
  const model = resultModel(
    ids.map((id) =>
      resultRow(id, {
        files: 2,
        bytes: '9007199254740993',
        blocked: true,
        availability: 'application_running',
      }),
    ),
  );
  render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  expect(screen.queryAllByRole('article')).toHaveLength(0);
  await openRetained(userEvent.setup());
  expect(screen.getAllByRole('article')).toHaveLength(ids.length);
  expect(model.decision.totals.bytes).toBe(0n);
  expect(model.decision.canPrepare).toBe(false);
  expect(
    screen.queryAllByRole('checkbox').filter((input) => !input.closest('.scan-volumes')),
  ).toHaveLength(0);
  expect(screen.queryByRole('button', { name: /重新检查.*状态/ })).not.toBeInTheDocument();
  for (const article of screen.getAllByRole('article')) {
    expect(article).toHaveTextContent(/版本|布局|依据|结构/);
    expect(article).toHaveTextContent(/保留|不支持|尚未开放/);
  }
});
it('shows actionable purpose choices directly and opens files without changing selection', async () => {
  const model = resultModel([
    resultRow('edge.http', {
      files: 1,
      bytes: '512',
      blocked: true,
      availability: 'browser_running',
    }),
    resultRow('weixin.logs', { blocked: true, availability: 'validation_pending' }),
    resultRow('chrome.http', { scan_issue: 'location_missing' }),
    resultRow('vivaldi.http', { files: 1, bytes: '128' }),
  ]);
  const user = userEvent.setup();
  const { rerender } = render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  expect(screen.getByRole('checkbox', { name: '选择浏览器缓存建议项' })).toBeVisible();
  expect(screen.queryByText('查看并调整')).not.toBeInTheDocument();
  expect(screen.queryAllByRole('article')).toHaveLength(0);
  await user.click(screen.getByRole('button', { name: '查看浏览器缓存建议项' }));
  expect(screen.getAllByRole('article')).toHaveLength(1);
  await user.click(screen.getByRole('button', { name: '查看Vivaldi 网页缓存的文件' }));
  expect(model.openDetail).toHaveBeenCalledTimes(1);
  expect(model.selectRuleGroup).not.toHaveBeenCalled();
  await openRetained(user, '浏览器缓存');
  expect(screen.getAllByRole('article')).toHaveLength(2);
  await user.click(screen.getByRole('button', { name: '重新检查应用状态' }));
  expect(model.refreshAvailability).toHaveBeenCalledOnce();
  expect(screen.queryByRole('button', { name: '查看全部检查范围' })).not.toBeInTheDocument();
  expect(screen.queryByText(/未发现候选的检查项/)).not.toBeInTheDocument();
  expect(screen.queryByText('Chrome 网页缓存')).not.toBeInTheDocument();
  expect(screen.queryByText('微信历史日志')).not.toBeInTheDocument();
  rerender(<CleanupDecisionPanel model={{ ...model, sessionKey: 'next' }} onReview={vi.fn()} />);
  expect(screen.queryAllByRole('article')).toHaveLength(0);
});
it('keeps a zero-result read failure with its item without a global check-count notice', async () => {
  const model = resultModel([
    resultRow('weixin.logs', {
      phase: 'partial',
      blocked: true,
      availability: 'validation_pending',
      scan_issue: 'access_denied',
      coverage: { complete: false, issues: [{ code: 'access_denied', count: 1 }] },
    }),
    resultRow('edge.http', {
      files: 2,
      bytes: '200',
      phase: 'partial',
      coverage: { complete: false, issues: [] },
    }),
    resultRow('qq.http', { scan_issue: 'location_missing', phase: 'failed', scan_failed: true }),
  ]);
  render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  expect(screen.queryByText(/类检查未完成/)).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '查看未完成的检查' })).not.toBeInTheDocument();
  const user = userEvent.setup();
  await openRetained(user, '日志与错误报告');
  const item = document.querySelector('[data-rule="weixin.logs"]') as HTMLElement;
  expect(item).toBeVisible();
  expect(document.querySelector('[data-rule="qq.http"]')).toBeNull();
  await user.click(within(item).getByText('技术详情'));
  await user.click(within(item).getByText(/读取问题/));
  expect(within(item).getByText('权限不足')).toBeVisible();
});
it('keeps technical conditions collapsed and cannot unlock readonly rules after refresh', async () => {
  const model = resultModel(
    ['weixin.logs', 'weixin.dumps'].map((id) => resultRow(id, { files: 1, bytes: '10' })),
  );
  model.decision.items.forEach((item) => {
    item.condition = '资料位置未获支持';
  });
  const user = userEvent.setup();
  const { rerender } = render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  await openRetained(user, '日志与错误报告');
  const articles = screen.getAllByRole('article');
  expect(within(articles[0]).getByText('资料位置未获支持')).not.toBeVisible();
  await user.click(within(articles[0]).getByText('技术详情'));
  expect(within(articles[0]).getByText('资料位置未获支持')).toBeVisible();
  expect(screen.queryByText(/存储管理指引/)).not.toBeInTheDocument();
  rerender(<CleanupDecisionPanel model={{ ...model }} onReview={vi.fn()} />);
  expect(
    screen.queryAllByRole('checkbox').filter((input) => !input.closest('.scan-volumes')),
  ).toHaveLength(0);
});

function acceptedDingTalk() {
  const catalog = rules.map((rule) => ({
    ...rule,
    cleanup_validated: rule.app_id === 'dingtalk' || rule.cleanup_validated,
  }));
  const applications = structuredClone(applicationFixtures);
  const app = applications.find((item) => item.app_id === 'dingtalk')!;
  app.running_components = [];
  for (const entry of app.rules!) {
    entry.cleanup_validated = true;
    entry.cleanup_versions = [app.version!];
    entry.cleanup_block = null;
  }
  return { catalog, applications };
}

it('shows selectable capacity for unselected accepted caches without counting readonly siblings', async () => {
  const { catalog, applications } = acceptedDingTalk();
  applications.find((app) => app.app_id === 'dingtalk')!.version = '99.0.0.1';
  const model = resultModel(
    [
      resultRow('dingtalk.http', { files: 2, bytes: '2048' }),
      resultRow('dingtalk.code', { files: 1, bytes: '1024' }),
      resultRow('baidu_netdisk.http', { files: 10, bytes: '9007199254740993' }),
    ],
    catalog,
    applications,
  );
  render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  expect(screen.getByRole('heading', { name: '发现可清理内容，请选择' })).toBeVisible();
  const summary = screen.getByText('可自行选择 · 可选 3.0 KB · 2 项');
  expect(summary).toBeVisible();
  await userEvent.setup().click(summary);
  expect(screen.getByText('3 个文件 · 可选 3.0 KB · 已选 0 个 / 0 B')).toBeVisible();
  expect(screen.getByRole('checkbox', { name: '选择应用缓存可自行选择项' })).not.toBeChecked();
  expect(model.decision.totals.bytes).toBe(0n);
  expect(screen.queryByRole('button', { name: '退出后重新检查' })).not.toBeInTheDocument();
});

it('rechecks only supported running caches and restores focus when the notice disappears', async () => {
  const { catalog, applications } = acceptedDingTalk();
  const selected = new Set(['dingtalk.http']);
  const rows = [
    resultRow('dingtalk.http', {
      files: 2,
      bytes: '2048',
      blocked: true,
      availability: 'application_running',
    }),
    resultRow('dingtalk.code', {
      files: 1,
      bytes: '1024',
      blocked: true,
      availability: 'process_unknown',
    }),
    resultRow('baidu_netdisk.http', {
      files: 10,
      bytes: '5000',
      blocked: true,
      availability: 'application_running',
    }),
  ];
  const model = resultModel(rows, catalog, applications, selected);
  const { rerender } = render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  expect(screen.getByText(/有 1 个清理项的相关应用仍在运行/)).toBeVisible();
  model.refreshAvailability = vi.fn(async () => {
    const next = resultModel(
      rows.map((row) =>
        row.rule_id === 'dingtalk.http' ? { ...row, blocked: false, availability: null } : row,
      ),
      catalog,
      applications,
      selected,
    );
    rerender(<CleanupDecisionPanel model={next} onReview={vi.fn()} />);
    expect(next.selected).toEqual(selected);
    expect(next.decision.totals.files).toBe(2);
  });
  await userEvent.setup().click(screen.getByRole('button', { name: '退出后重新检查' }));
  expect(model.refreshAvailability).toHaveBeenCalledOnce();
  expect(model.selectRuleGroup).not.toHaveBeenCalled();
  expect(screen.queryByRole('button', { name: '退出后重新检查' })).not.toBeInTheDocument();
  expect(document.querySelector('.desk-results')).toHaveFocus();
});

it.each([null, 'application_running'] as const)(
  'preserves layout refusals across version changes, even with scan status %s',
  (availability) => {
    const { catalog, applications } = acceptedDingTalk();
    applications.find((app) => app.app_id === 'dingtalk')!.version = '99.0.0.1';
    applications
      .find((app) => app.app_id === 'dingtalk')!
      .rules!.find((rule) => rule.rule_id === 'dingtalk.http')!.cleanup_block =
      'unsupported_layout';
    const model = resultModel(
      [
        resultRow('dingtalk.http', {
          files: 2,
          bytes: '2048',
          availability,
          blocked: !!availability,
        }),
        resultRow('baidu_netdisk.http', {
          files: 1,
          bytes: '1024',
          blocked: true,
          availability: 'application_running',
        }),
      ],
      catalog,
      applications,
    );
    render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
    expect(screen.getByRole('heading', { name: '本次发现的内容暂不清理' })).toBeVisible();
    expect(screen.queryByText(/可自行选择 ·/)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '退出后重新检查' })).not.toBeInTheDocument();
  },
);

it('offers a supported running-app recheck even when no candidate was readable', async () => {
  const { catalog, applications } = acceptedDingTalk();
  const model = resultModel(
    [
      resultRow('dingtalk.http', {
        files: 0,
        bytes: '0',
        blocked: true,
        availability: 'application_running',
      }),
    ],
    catalog,
    applications,
  );
  model.refreshAvailability = vi.fn(async () => undefined);
  render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  expect(screen.getByRole('heading', { name: '部分清理项需退出应用后检查' })).toBeVisible();
  const notice = screen.getByRole('region', { name: '退出应用后可重新检查' });
  expect(
    within(notice).getByText(catalog.find((r) => r.id === 'dingtalk.http')!.title),
  ).toBeVisible();
  await userEvent.setup().click(within(notice).getByRole('button', { name: '退出后重新检查' }));
  expect(model.refreshAvailability).toHaveBeenCalledOnce();
  expect(model.selectRuleGroup).not.toHaveBeenCalled();
});

it('offers process-state retry with zero candidates and keeps selection and focus', async () => {
  const { catalog, applications } = acceptedDingTalk();
  const selected = new Set(['dingtalk.http']);
  const rows = [resultRow('dingtalk.http', { blocked: true, availability: 'process_unknown' })];
  const model = resultModel(rows, catalog, applications, selected);
  const { rerender } = render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  expect(screen.getByRole('heading', { name: '部分清理项的运行状态尚未确认' })).toBeVisible();
  expect(screen.queryByRole('button', { name: '退出后重新检查' })).not.toBeInTheDocument();
  model.refreshAvailability = vi.fn(async () => {
    const next = resultModel([resultRow('dingtalk.http')], catalog, applications, selected);
    rerender(<CleanupDecisionPanel model={next} onReview={vi.fn()} />);
    expect(next.selected).toEqual(selected);
    expect(next.decision.totals.files).toBe(0);
  });
  await userEvent.setup().click(screen.getByRole('button', { name: '重新检查运行状态' }));
  expect(model.refreshAvailability).toHaveBeenCalledOnce();
  expect(document.querySelector('.desk-results')).toHaveFocus();
});

it('never offers runtime retry to unlock a readonly zero-candidate rule', () => {
  const model = resultModel([
    resultRow('baidu_netdisk.http', { blocked: true, availability: 'process_unknown' }),
  ]);
  render(<CleanupDecisionPanel model={model} onReview={vi.fn()} />);
  expect(screen.queryByRole('button', { name: '重新检查运行状态' })).not.toBeInTheDocument();
  expect(screen.getByRole('heading', { name: '这次没有发现可直接清理的垃圾' })).toBeVisible();
  expect(screen.queryByText(/百度网盘/)).not.toBeInTheDocument();
  expect(model.decision.totals.bytes).toBe(0n);
});

it.each([
  [{ scan_issue: 'location_missing' as const }, '这次没有发现可直接清理的垃圾'],
  [
    {
      phase: 'partial' as const,
      coverage: { complete: false, issues: [{ code: 'access_denied' as const, count: 1 }] },
    },
    '本次发现的内容暂不清理',
  ],
  [{}, '这次没有发现可直接清理的垃圾'],
])('distinguishes zero-result coverage states: %s', (patch, title) => {
  render(
    <CleanupDecisionPanel
      model={resultModel([resultRow('windows.user_temp', patch)])}
      onReview={vi.fn()}
    />,
  );
  expect(screen.getByRole('heading', { name: title })).toBeVisible();
});
