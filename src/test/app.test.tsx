import { render, renderHook, act, screen, waitFor, cleanup, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import { useAppController } from '../useAppController';
import { formatBytes } from '../i18n';
import rules from '../previewRules';
import { applicationFixtures } from './applicationFixtures';
import type { Report, RuleOutcome } from '../types';
const systemVolume = {
  token: 'system-volume',
  title: '系统磁盘 C:',
  path: 'C:\\',
  kind: 'volume' as const,
  is_system: true,
  supported: true,
};

const mock = vi.hoisted(() => ({
  windowsSnapshot: vi.fn(),
  windowsScan: vi.fn(),
  windowsPrepare: vi.fn(),
  windowsCancel: vi.fn(),
  softwareList: vi.fn(),
  manualPreview: vi.fn(),
  cancelManualPreview: vi.fn(),
  cancelPreparation: vi.fn(),
  excludeAnalysis: vi.fn(),
  bootstrap: vi.fn(),
  applicationSupport: vi.fn(),
  attach: vi.fn(),
  onCoordination: vi.fn(),
  snapshot: vi.fn(),
  addExclusions: vi.fn(),
  reveal: vi.fn(),
  scan: vi.fn(),
  status: vi.fn(),
  summary: vi.fn(),
  report: vi.fn(),
  prepare: vi.fn(),
  execute: vi.fn(),
  history: vi.fn(),
  cancel: vi.fn(),
  save: vi.fn(),
  candidates: vi.fn(),
  refresh: vi.fn(),
  refreshRule: vi.fn(),
  uvTask: vi.fn(),
  toolTask: vi.fn(),
  analysisLocations: vi.fn(),
  analysisContext: vi.fn(),
  analysisChildren: vi.fn(),
  analysisDiscovery: vi.fn(),
  analysisStorage: vi.fn().mockResolvedValue('C:\\fixture\\analysis-cache'),
  pickAnalysisStorage: vi.fn(),
}));
vi.mock('../api', () => ({ desktop: true, api: mock }));
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 126,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({ index, start: index * 126 })),
    measureElement: () => {},
    measure: () => {},
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onResized: vi.fn().mockResolvedValue(() => {}),
    isMaximized: vi.fn().mockResolvedValue(false),
  }),
}));
async function startDefaultScan(user: ReturnType<typeof userEvent.setup>) {
  const scan = await screen.findByRole('button', { name: '开始扫描' });
  await waitFor(() => expect(scan).toBeEnabled(), { timeout: 5000 });
  await user.click(scan);
}
async function openAdjustments(user: ReturnType<typeof userEvent.setup>) {
  await screen.findByRole('region', { name: '建议清理' });
  for (const summary of document.querySelectorAll<HTMLElement>('.result-section > summary'))
    if (!summary.closest('details')?.open) await user.click(summary);
  for (const button of document.querySelectorAll<HTMLElement>(
    '.purpose-toggle[aria-expanded="false"]',
  ))
    await user.click(button);
}
async function openWorkbench(user: ReturnType<typeof userEvent.setup>) {
  await openAdjustments(user);
}
beforeEach(() => {
  vi.clearAllMocks();
  Object.values(mock).forEach((fn) => fn.mockReset());
  mock.analysisStorage.mockResolvedValue('C:\\fixture\\analysis-cache');
  localStorage.clear();
  mock.softwareList.mockResolvedValue({ items: [], warnings: [] });
  mock.applicationSupport.mockResolvedValue(structuredClone(applicationFixtures));
  mock.attach.mockResolvedValue(null);
  mock.onCoordination.mockReturnValue(() => {});
  mock.bootstrap.mockResolvedValue({
    rules: rules.filter((r) => !r.requires_admin),
    settings: { theme: 'system', exclusions: [], excluded_rules: [] },
    version: '0.3.0',
    preferences: { last_scan_rule_ids: null },
    capabilities: { system_cleanup: false },
  });
  mock.analysisLocations.mockResolvedValue([systemVolume]);
  mock.windowsSnapshot.mockResolvedValue(null);
  mock.analysisDiscovery.mockImplementation(async () => {
    const p = await mock.analysisChildren();
    return p ?? { items: [], total: 0, offset: 0 };
  });
  mock.analysisContext.mockResolvedValue({
    location_token: 'system-volume',
    volume: 'C:\\',
    volume_key: 'system',
    available_bytes: '1000000',
    sampled_at: 1,
    error: null,
  });
  mock.cancelManualPreview.mockResolvedValue(undefined);
  mock.manualPreview.mockImplementation(async (selection, request_id) => ({
    request_id,
    session: selection.session,
    files: 2,
    directories: 1,
    bytes: '9007199254740993',
    retained: 1,
  }));
  mock.history.mockResolvedValue([]);
  mock.uvTask.mockResolvedValue(null);
  mock.toolTask.mockResolvedValue(null);
  mock.save.mockImplementation(async (patch) => ({
    scan_invalidated: false,
    settings: { theme: 'system', exclusions: [], excluded_rules: [], ...patch },
  }));
  mock.cancel.mockResolvedValue(null);
  mock.candidates.mockResolvedValue({
    total: 3,
    offset: 0,
    items: Array.from({ length: 3 }, (_, i) => ({
      id: `file-${i}`,
      rule_id: 'windows.user_temp',
      path: `C:/fixture/${i}.tmp`,
      bytes: '1024',
      modified_unix: 0,
      status: 'candidate',
      reason: null,
      reason_code: null,
    })),
  });
  mock.report.mockResolvedValue(null);
  mock.scan.mockResolvedValue({ sessions: [{ scope: 'user', id: 'scan-1' }], warning: null });
  mock.status.mockResolvedValue({
    id: 'scan-1',
    state: 'ready',
    visited: 3,
    completed: 0,
    total: 0,
    current_rule: '',
    error: null,
  });
  mock.summary.mockResolvedValue({
    session_id: 'scan-1',
    state: 'ready',
    rules: [
      {
        phase: 'completed',
        rule_id: 'windows.user_temp',
        files: 3,
        bytes: '3072',
        skipped: 0,
        blocked: false,
        note: null,
        scan_failed: false,
        scan_issue: null,
        availability: null,
        reasons: [],
      },
    ],
  });
  mock.prepare.mockResolvedValue([
    {
      scope: 'user',
      plan: {
        id: 'plan-1',
        session_id: 'scan-1',
        files: 3,
        bytes: '3072',
        rule_ids: ['windows.user_temp'],
        impacts: ['保留用户文件'],
        categories: [{ rule_id: 'windows.user_temp', files: 3, bytes: '3072' }],
      },
    },
  ]);
  mock.execute.mockResolvedValue({ scopes: ['user'], warning: null });
  mock.snapshot.mockImplementation(async () => {
    if (!mock.scan.mock.calls.length)
      return {
        status: {
          id: '',
          state: 'idle',
          visited: 0,
          completed: 0,
          total: 0,
          current_rule: '',
          error: null,
        },
        summary: { session_id: '', state: 'idle', rules: [] },
        report: null,
      };
    return {
      status: await mock.status(),
      summary: await mock.summary(),
      report: await mock.report(),
    };
  });
});
afterEach(cleanup);

it('keeps user choices when system authorization is cancelled and never selects system rules automatically', async () => {
  const initial = await mock.bootstrap();
  mock.bootstrap.mockResolvedValue({ ...initial, rules, capabilities: { system_cleanup: true } });
  const hook = renderHook(() => useAppController());
  await waitFor(() =>
    expect(hook.result.current.canScanVolumes && !hook.result.current.busy).toBe(true),
  );
  await act(() => hook.result.current.scan());
  await waitFor(() => expect(hook.result.current.selected.has('windows.user_temp')).toBe(true));
  const before = [...hook.result.current.selected];
  mock.scan.mockResolvedValueOnce({ sessions: [], warning: '系统授权已取消' });
  await act(() => hook.result.current.scanSystem());
  expect([...hook.result.current.selected]).toEqual(before);
  expect(hook.result.current.sessions).toEqual([{ scope: 'user', id: 'scan-1' }]);
  expect(mock.scan).toHaveBeenLastCalledWith(['windows.system_temp', 'windows.iis_logs']);
  expect(hook.result.current.selected.has('windows.system_temp')).toBe(false);
});

it('confirms a Windows-only plan separately without a file plan or default selection', async () => {
  const action = {
    id: 'windows.thumbnail',
    title: '缩略图缓存',
    scope: 'user',
    impact: '预览会重新生成',
    estimate_bytes: '4096',
    volume: 'C:\\',
    available: true,
    validated: true,
    message: 'Windows 官方估算',
  };
  let state = {
    session: 'win-one',
    status: 'ready',
    actions: [action],
    plan: null as unknown,
    current: null,
    report: null,
    message: '',
  };
  mock.windowsSnapshot.mockImplementation(async () => state);
  mock.windowsPrepare.mockImplementation(async () => {
    state = {
      ...state,
      status: 'prepared',
      plan: { id: 'win-plan', session: 'win-one', actions: [action], expires_at: 2000000000 },
    };
  });
  const row = await mock.summary();
  mock.summary.mockResolvedValue({ ...row, rules: [{ ...row.rules[0], files: 0, bytes: '0' }] });
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await user.click(await screen.findByText('Windows 官方维护 · 单独估算与确认'));
  const checkbox = await screen.findByRole('checkbox', { name: /缩略图缓存/ });
  expect(checkbox).not.toBeChecked();
  await user.click(checkbox);
  await user.click(await screen.findByRole('button', { name: /^清理已选/ }));
  expect(await screen.findByRole('alertdialog')).toHaveTextContent('已选 Windows 维护项目');
  expect(mock.prepare).not.toHaveBeenCalled();
  expect(mock.execute).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: '永久删除' }));
  await waitFor(() => expect(mock.execute).toHaveBeenCalledTimes(1));
  expect(mock.execute.mock.calls[0][0]).toEqual([]);
  expect(mock.execute.mock.calls[0][1][0].plan.id).toBe('win-plan');
});

it('defaults to C only and submits explicit single or multiple disks without automatically scanning', async () => {
  const d = {
    ...systemVolume,
    token: 'data-volume',
    path: 'D:\\',
    title: '磁盘 D:',
    is_system: false,
  };
  mock.analysisLocations.mockResolvedValue([systemVolume, d]);
  const user = userEvent.setup();
  render(<App />);
  const c = await screen.findByRole('checkbox', { name: '扫描 C:\\' });
  await waitFor(() => expect(c).toBeChecked());
  expect(screen.getByRole('checkbox', { name: '扫描 D:\\' })).not.toBeChecked();
  expect(mock.scan).not.toHaveBeenCalled();
  await user.click(screen.getByRole('checkbox', { name: '扫描 D:\\' }));
  await user.click(screen.getByRole('button', { name: '开始扫描' }));
  await waitFor(() =>
    expect(mock.scan).toHaveBeenCalledWith(expect.any(Array), ['system-volume', 'data-volume']),
  );
});
it('allows all disks and blocks an empty or unavailable disk selection', async () => {
  mock.analysisLocations.mockResolvedValue([
    { ...systemVolume, token: 'data-volume', path: 'D:\\', is_system: false },
  ]);
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole('checkbox', { name: '扫描 D:\\' });
  expect(screen.getByRole('button', { name: '开始扫描' })).toBeDisabled();
  await user.click(screen.getByRole('checkbox', { name: '全部磁盘' }));
  expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled();
  await user.click(screen.getByRole('checkbox', { name: '全部磁盘' }));
  expect(screen.getByRole('button', { name: '开始扫描' })).toBeDisabled();
  await user.click(screen.getByRole('checkbox', { name: '全部磁盘' }));
  await user.click(screen.getByRole('button', { name: '开始扫描' }));
  await waitFor(() => expect(mock.scan).toHaveBeenCalledWith(expect.any(Array)));
});

async function restoreReport(categories: RuleOutcome[], patch: Partial<Report> = {}) {
  const job = await mock.status(),
    summary = await mock.summary();
  const counts = categories.reduce(
    (n, c) => ({
      total: n.total + c.total,
      deleted: n.deleted + c.deleted,
      skipped: n.skipped + c.skipped,
      failed: n.failed + c.failed,
      unknown: n.unknown + c.unknown,
    }),
    { total: 0, deleted: 0, skipped: 0, failed: 0, unknown: 0 },
  );
  mock.status.mockResolvedValue({ ...job, state: 'finished' });
  mock.summary.mockResolvedValue({
    ...summary,
    state: 'finished',
    rules: categories.map((c) => ({ ...summary.rules[0], rule_id: c.rule_id })),
  });
  mock.report.mockResolvedValue({
    id: 'result-1',
    scope: 'user',
    status: 'finished',
    started_at: 1,
    finished_at: 2,
    candidate_bytes: '3072',
    deleted_bytes: '1024',
    free_space_change: null,
    categories,
    ...counts,
    ...patch,
  });
  mock.snapshot.mockImplementation(async () => ({
    status: await mock.status(),
    summary: await mock.summary(),
    report: await mock.report(),
  }));
}
const outcome = (patch: Partial<RuleOutcome> = {}): RuleOutcome => ({
  rule_id: 'windows.user_temp',
  total: 2,
  deleted: 1,
  skipped: 0,
  failed: 1,
  unknown: 0,
  deleted_bytes: '1024',
  reasons: [{ code: 'access_denied', count: 1 }],
  retained: [],
  ...patch,
});

it('opens uncertain details through the cleanup record and restores its trigger', async () => {
  await restoreReport([outcome({ total: 3, unknown: 1 })]);
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole('button', { name: '查看清理记录' }));
  const trigger = within(screen.getByLabelText('按结果查看文件')).getByRole('button', {
    name: '状态不确定 1 项',
  });
  await user.click(trigger);
  await screen.findByRole('dialog');
  await waitFor(() =>
    expect(mock.candidates).toHaveBeenLastCalledWith(
      'user',
      'scan-1',
      'windows.user_temp',
      0,
      expect.objectContaining({ view: 'cleanup', status: 'unknown', search: '' }),
    ),
  );
  await user.click(screen.getByRole('button', { name: '关闭文件明细' }));
  await waitFor(() => expect(trigger).toHaveFocus());
  await user.click(
    within(screen.getByLabelText('按结果查看文件')).getByRole('button', {
      name: /失败 1 项/,
    }),
  );
  await waitFor(() =>
    expect(mock.candidates).toHaveBeenLastCalledWith(
      'user',
      'scan-1',
      'windows.user_temp',
      0,
      expect.objectContaining({ status: 'failed' }),
    ),
  );
  expect(mock.prepare).not.toHaveBeenCalled();
  expect(mock.execute).not.toHaveBeenCalled();
});
it('orders multiple matching categories, preserves review context and clears it after a new scan', async () => {
  await restoreReport([outcome(), outcome({ rule_id: 'chrome.http', total: 5, failed: 4 })]);
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByText('查看清理记录', { selector: 'summary' }));
  await user.click(
    within(await screen.findByLabelText('按结果查看文件')).getByRole('button', {
      name: /失败 [0-9]+ 项/,
    }),
  );
  const panel = await screen.findByRole('region', { name: '失败类别' });
  const items = within(panel).getAllByRole('listitem');
  expect(items[0]).toHaveTextContent('Chrome 网页缓存 · 4 个文件');
  expect(items[1]).toHaveTextContent('用户临时文件 · 1 个文件');
  expect(mock.candidates).not.toHaveBeenCalled();
  const trigger = within(items[0]).getByRole('button');
  await user.click(trigger);
  await waitFor(() =>
    expect(mock.candidates).toHaveBeenLastCalledWith(
      'user',
      'scan-1',
      'chrome.http',
      0,
      expect.objectContaining({ status: 'failed', search: '' }),
    ),
  );
  await user.type(screen.getByRole('searchbox'), 'old search');
  await user.click(screen.getByRole('button', { name: '关闭文件明细' }));
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(panel).toBeVisible();
  await user.click(within(items[1]).getByRole('button'));
  expect(screen.getByRole('searchbox')).toHaveValue('');
  await waitFor(() =>
    expect(mock.candidates).toHaveBeenLastCalledWith(
      'user',
      'scan-1',
      'windows.user_temp',
      0,
      expect.objectContaining({ status: 'failed', search: '' }),
    ),
  );
  await user.click(screen.getByRole('button', { name: '关闭文件明细' }));
  const status = await mock.status(),
    summary = await mock.summary();
  mock.scan.mockImplementationOnce(async () => {
    mock.status.mockResolvedValue({ ...status, id: 'scan-2', state: 'ready' });
    mock.summary.mockResolvedValue({ ...summary, session_id: 'scan-2', state: 'ready' });
    mock.report.mockResolvedValue(null);
    return { sessions: [{ scope: 'user', id: 'scan-2' }], warning: null };
  });
  await user.click(screen.getByRole('button', { name: '完成' }));
  await user.click(screen.getByRole('button', { name: '开始扫描' }));
  await waitFor(() =>
    expect(screen.queryByRole('region', { name: '失败类别' })).not.toBeInTheDocument(),
  );
  expect(mock.execute).not.toHaveBeenCalled();
});
it('finishes normal retention while keeping a filtered file entry', async () => {
  await restoreReport([
    outcome({ failed: 0, skipped: 1, reasons: [{ code: 'protected', count: 1 }] }),
  ]);
  const user = userEvent.setup();
  render(<App />);
  const panel = await screen.findByRole('region', { name: '清理决策总览' });
  await within(panel).findByRole('heading', { name: '清理完成' });
  expect(within(panel).getByRole('button', { name: '完成' })).toBeVisible();
  expect(screen.queryByText('本次计划已全部删除')).not.toBeInTheDocument();
  expect(within(panel).getByText('按规则保留 1 项')).toBeVisible();
  await user.click(screen.getByText('查看清理记录', { selector: 'summary' }));
  await user.click(
    within(screen.getByLabelText('按结果查看文件')).getByRole('button', {
      name: /按规则保留 1 项/,
    }),
  );
  await waitFor(() =>
    expect(mock.candidates).toHaveBeenLastCalledWith(
      'user',
      'scan-1',
      'windows.user_temp',
      0,
      expect.objectContaining({ status: 'skipped' }),
    ),
  );
});
it('keeps statistics when report details do not belong to the current scan', async () => {
  await restoreReport([outcome()]);
  const summary = await mock.summary();
  mock.summary.mockResolvedValue({ ...summary, rules: [] });
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByText('查看清理记录', { selector: 'summary' }));
  await user.click(
    within(await screen.findByLabelText('按结果查看文件')).getByRole('button', {
      name: /失败 [0-9]+ 项/,
    }),
  );
  expect(
    within(await screen.findByRole('region', { name: '失败类别' })).getByText(
      /当前会话明细不可用，仅保留统计/,
    ),
  ).toBeVisible();
  expect(mock.candidates).not.toHaveBeenCalled();
});

it('samples space on the cleanup home without starting analysis, and refreshes in place', async () => {
  const location = {
    token: 'system',
    kind: 'volume',
    title: '系统盘',
    path: 'C:\\',
    is_system: true,
    supported: true,
  };
  mock.analysisLocations.mockResolvedValue([location]);
  mock.analysisContext.mockResolvedValue({
    location_token: 'system',
    volume: 'C:\\',
    volume_key: 'volume',
    available_bytes: '1000',
    sampled_at: 1,
    error: null,
  });
  const user = userEvent.setup();
  render(<App />);
  await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled(), {
    timeout: 5000,
  });
  const space = await screen.findByRole('region', { name: '磁盘可用空间' });
  await within(space).findByText('1000 B');
  expect(within(space).getByText('当前可用')).toBeVisible();
  expect(mock.analysisContext).toHaveBeenCalledTimes(1);
  expect(mock.scan).not.toHaveBeenCalled();
  mock.analysisContext.mockResolvedValue({
    location_token: 'system',
    volume: 'C:\\',
    volume_key: 'volume',
    available_bytes: '900',
    sampled_at: 2,
    error: null,
  });
  await user.click(within(space).getByRole('button', { name: '刷新磁盘空间' }));
  await within(space).findByText('900 B');
  expect(mock.analysisContext).toHaveBeenCalledTimes(2);
  expect(screen.getByRole('button', { name: '清理' })).toHaveAttribute('aria-current', 'page');
  expect(mock.analysisChildren).not.toHaveBeenCalled();
});

it('shows mixed app decisions and inspecting a category does not change the selection or plan', async () => {
  const boot = await mock.bootstrap();
  const ids = ['windows.user_temp', 'edge.http', 'weixin.logs', 'chrome.http'];
  mock.bootstrap.mockResolvedValue({ ...boot, preferences: { last_scan_rule_ids: ids } });
  const summary = await mock.summary();
  mock.summary.mockResolvedValue({
    ...summary,
    rules: [
      summary.rules[0],
      { ...summary.rules[0], rule_id: 'edge.http', blocked: true, availability: 'browser_running' },
      {
        ...summary.rules[0],
        rule_id: 'weixin.logs',
        blocked: true,
        availability: 'validation_pending',
      },
      {
        ...summary.rules[0],
        rule_id: 'chrome.http',
        phase: 'failed',
        blocked: true,
        scan_failed: true,
      },
    ],
  });
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await openAdjustments(user);
  const panel = screen.getByRole('region', { name: '清理决策总览' });
  await within(panel).findByRole('button', { name: '查看临时文件建议项' });
  expect(within(panel).getByText('本次未清理 · 2 类')).toBeVisible();
  const weixin = screen.getByRole('region', { name: '微信专项清理' });
  expect(document.querySelector('.task-dock')).toHaveTextContent('已选 3.0 KB · 3 个文件');
  expect(panel.querySelector('.desk-amount')).toBeNull();
  const chosen = () =>
    Array.from(document.querySelectorAll<HTMLInputElement>('.rule-list input:checked'))
      .map((e) => e.id)
      .sort();
  const before = chosen();
  await user.click(within(weixin).getByRole('button', { name: '微信专项清理' }));
  expect(within(weixin).getAllByText('尚未开放清理')[0]).toBeVisible();
  expect(chosen()).toEqual(before);
  expect(mock.prepare).not.toHaveBeenCalled();
  await user.click(within(weixin).getByRole('button', { name: '微信专项清理' }));
  expect(within(panel).queryByText(/存储管理指引/)).not.toBeInTheDocument();
  expect(chosen()).toEqual(before);
  await user.click(screen.getByRole('button', { name: /^清理已选/ }));
  expect(mock.prepare.mock.calls[0][0][0].selection.rule_ids).toEqual(['windows.user_temp']);
});

it('never asks to quit readonly applications, and preserves selection across support refresh', async () => {
  const boot = await mock.bootstrap();
  const ids = ['weixin.logs', 'weixin.dumps'];
  mock.bootstrap.mockResolvedValue({ ...boot, preferences: { last_scan_rule_ids: ids } });
  const summary = await mock.summary();
  const updated = {
    ...summary,
    rules: ids.map((rule_id) => ({
      ...summary.rules[0],
      rule_id,
      blocked: true,
      availability: 'application_running',
      note: '请完全退出后重新检查',
    })),
  };
  mock.summary.mockResolvedValue(updated);
  mock.refresh.mockResolvedValue(updated);
  const apps = structuredClone(applicationFixtures);
  apps[0].status = 'application_running';
  apps[0].running_components = ['Weixin.exe'];
  mock.applicationSupport.mockResolvedValue(apps);
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await openAdjustments(user);
  const panel = screen.getByRole('region', { name: '微信专项清理' });
  await user.click(within(panel).getByRole('button', { name: '微信专项清理' }));
  expect((await within(panel).findAllByText('尚未开放清理')).length).toBeGreaterThanOrEqual(2);
  expect(within(panel).getAllByText(/仍在运行：Weixin.exe/)[0]).not.toBeVisible();
  expect(screen.queryByRole('button', { name: '我已关闭，重新检查' })).not.toBeInTheDocument();
  expect(within(panel).queryByRole('button', { name: '重新检查微信状态' })).not.toBeInTheDocument();
  expect(
    screen.getAllByText(/仅检查至少七天/).some((e) => e.textContent?.includes('聊天记录与附件')),
  ).toBe(true);
  expect(screen.queryByText('请完全退出后重新检查')).not.toBeInTheDocument();
  const logs = panel.querySelector('[data-rule="weixin.logs"]') as HTMLElement;
  await user.click(within(logs).getByText('技术详情'));
  await user.click(within(logs).getByRole('button', { name: '重新核对应用信息' }));
  await waitFor(() => expect(mock.applicationSupport).toHaveBeenCalledTimes(2));
  for (const id of ids) expect(document.getElementById(`choose-${id}`)).toBeNull();
  expect(mock.scan).toHaveBeenCalledTimes(1);
  expect(mock.prepare).not.toHaveBeenCalled();
});

it('keeps available rules usable when application support fails', async () => {
  const boot = await mock.bootstrap();
  mock.bootstrap.mockResolvedValue({
    ...boot,
    preferences: { last_scan_rule_ids: ['windows.user_temp', 'weixin.logs'] },
  });
  mock.applicationSupport.mockRejectedValue(new Error('support unavailable'));
  const summary = await mock.summary();
  mock.summary.mockResolvedValue({
    ...summary,
    rules: [summary.rules[0], { ...summary.rules[0], rule_id: 'weixin.logs' }],
  });
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  const panel = screen.getByRole('region', { name: '微信专项清理' });
  await user.click(within(panel).getByRole('button', { name: '微信专项清理' }));
  expect(within(panel).getByText('微信历史运行日志')).toBeVisible();
  expect(document.querySelector('.task-dock')).toHaveTextContent('已选 3.0 KB · 3 个文件');
  await user.click(screen.getByRole('button', { name: /^清理已选/ }));
  expect(mock.prepare.mock.calls[0][0][0].selection.rule_ids).toEqual(['windows.user_temp']);
});

it('preserves file selection through folios, view changes, pages and motion preferences', async () => {
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await openWorkbench(user);
  const details = await screen.findByRole('button', { name: '查看用户临时文件的文件' });
  await user.click(details);
  await user.click(await screen.findByRole('button', { name: '取消本页选择' }));
  await user.click(screen.getByRole('button', { name: '关闭文件明细' }));
  await waitFor(() => expect(details).toHaveFocus());
  await openWorkbench(user);
  await user.click(screen.getByRole('button', { name: '查看临时文件建议项' }));
  await user.click(screen.getByRole('button', { name: '查看临时文件建议项' }));
  await user.click(screen.getByRole('button', { name: '设置' }));
  await user.click(screen.getByRole('button', { name: '减少动效' }));
  await user.click(screen.getByRole('button', { name: '历史' }));
  await user.click(screen.getByRole('button', { name: '清理' }));
  expect(screen.getByRole('checkbox', { name: '用户临时文件' })).not.toBeChecked();
  expect(screen.queryByRole('slider')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /^清理已选/ })).not.toBeInTheDocument();
  expect(mock.scan).toHaveBeenCalledTimes(1);
  expect(mock.prepare).not.toHaveBeenCalled();
  expect(mock.execute).not.toHaveBeenCalled();
  expect(JSON.parse(localStorage.getItem('dustaway.ui.v3')!)).toEqual({
    motion: 'reduced',
  });
});

it('group selection is mixed and cannot include an excluded rule', async () => {
  const boot = await mock.bootstrap();
  boot.settings.excluded_rules = ['edge.code'];
  mock.bootstrap.mockResolvedValue(boot);
  const summary = await mock.summary();
  mock.summary.mockResolvedValue({
    ...summary,
    rules: ['edge.http', 'edge.gpu'].map((rule_id) => ({ ...summary.rules[0], rule_id })),
  });
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await screen.findByRole('button', { name: /^清理已选/ });
  await openWorkbench(user);
  await user.click(screen.getByRole('checkbox', { name: 'Edge 图形缓存' }));
  const group = screen.getByRole('checkbox', { name: '选择浏览器缓存建议项' });
  expect(group).toBePartiallyChecked();
  await user.click(group);
  expect(group).toBeChecked();
  await user.click(screen.getByRole('button', { name: /^清理已选/ }));
  await waitFor(() =>
    expect(mock.prepare.mock.calls[0][0][0].selection.rule_ids).toEqual(['edge.http', 'edge.gpu']),
  );
  expect(mock.scan.mock.calls[0][0]).not.toContain('edge.code');
});

it('allows partial verified candidates and carries backend coverage into confirmation', async () => {
  const summary = await mock.summary();
  summary.rules[0].phase = 'partial';
  summary.rules[0].coverage = { complete: false, issues: [{ code: 'access_denied', count: 1 }] };
  mock.summary.mockResolvedValue(summary);
  const plans = await mock.prepare();
  plans[0].plan.categories[0].coverage = summary.rules[0].coverage;
  mock.prepare.mockResolvedValue(plans);
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await openAdjustments(user);
  expect(screen.queryByText(/类检查未完成/)).not.toBeInTheDocument();
  expect(
    within(screen.getByLabelText('用户临时文件').closest('.rule-row') as HTMLElement).getByText(
      /扫描结果不完整；清理范围/,
    ),
  ).not.toBeVisible();
  await user.click(await screen.findByRole('button', { name: /^清理已选/ }));
  const dialog = await screen.findByRole('alertdialog');
  expect(dialog).toHaveTextContent('扫描结果不完整');
  expect(screen.getByRole('button', { name: '永久删除' })).toBeEnabled();
  expect(mock.execute).not.toHaveBeenCalled();
});

it('scans all supported user rules without selecting files before scanning', async () => {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole('button', { name: '开始扫描' });
  expect(screen.queryByRole('button', { name: 'Brave' })).not.toBeInTheDocument();
  expect(screen.queryByRole('checkbox', { name: /Brave 网页缓存/ })).not.toBeInTheDocument();
  await startDefaultScan(user);
  await waitFor(() => expect(mock.scan).toHaveBeenCalled());
  expect(mock.scan.mock.calls[0][0]).toEqual(
    rules.filter((r) => !r.requires_admin).map((r) => r.id),
  );
});

it('keeps discovered cache residues selectable even when the software is no longer installed', async () => {
  const summary = await mock.summary();
  mock.summary.mockResolvedValue({
    ...summary,
    rules: [summary.rules[0], { ...summary.rules[0], rule_id: 'firefox.http' }],
  });
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await screen.findByRole('button', { name: /^清理已选/ });
  await openAdjustments(user);
  expect(mock.scan.mock.calls[0][0]).toContain('firefox.http');
  expect(mock.softwareList).not.toHaveBeenCalled();
  expect(
    screen.getByRole('checkbox', { name: rules.find((rule) => rule.id === 'firefox.http')!.title }),
  ).toBeChecked();
  expect(document.querySelector('.task-dock')).toHaveTextContent('已选 6.0 KB · 6 个文件');
  await user.click(screen.getByRole('button', { name: /^清理已选/ }));
  await waitFor(() => expect(mock.prepare).toHaveBeenCalled());
  expect(mock.prepare.mock.calls[0][0][0].selection.rule_ids).toEqual([
    'windows.user_temp',
    'firefox.http',
  ]);
});

it('comprehensive scanning ignores old narrowed preferences and respects exclusions', async () => {
  mock.bootstrap.mockResolvedValue({
    rules: rules.filter((r) => !r.requires_admin),
    settings: { theme: 'dark', exclusions: ['C:\\protected'], excluded_rules: ['edge.code'] },
    version: '0.6.0',
    preferences: { last_scan_rule_ids: ['edge.http', 'edge.code'] },
    capabilities: { system_cleanup: false },
  });
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await waitFor(() =>
    expect(mock.scan).toHaveBeenCalledWith(
      rules.filter((r) => !r.requires_admin && r.id !== 'edge.code').map((r) => r.id),
      ['system-volume'],
    ),
  );
  expect(mock.save).not.toHaveBeenCalled();
});
describe('user-controlled cleanup', () => {
  it('shows one scan action and only discovered results without a rule catalog', async () => {
    const user = userEvent.setup();
    render(<App />);
    const scan = await screen.findByRole('button', { name: '开始扫描' });
    expect(screen.getAllByRole('button', { name: '开始扫描' })).toHaveLength(1);
    expect(scan.closest('.cleanup-decision')).not.toBeNull();
    expect(screen.getByRole('heading', { name: '把空间，慢慢展开。' })).toBeVisible();
    expect(screen.queryByRole('button', { name: '浏览器缓存' })).not.toBeInTheDocument();
    expect(document.querySelector('.scan-scope')).toBeNull();
    expect(document.querySelector('.action-bar')).not.toBeVisible();
    expect(screen.getByRole('region', { name: '清理决策总览' })).not.toHaveTextContent('23');
    expect(screen.queryByRole('button', { name: '查看扫描范围' })).not.toBeInTheDocument();
    expect(screen.queryByText('Brave 网页缓存')).not.toBeInTheDocument();
    await waitFor(() => expect(scan).toBeEnabled());
    await user.click(scan);
    await screen.findByRole('button', { name: /^清理已选/ });
    expect(screen.getByLabelText('用户临时文件')).not.toBeVisible();
    expect(screen.getByRole('checkbox', { name: '选择临时文件建议项' })).toBeVisible();
    await openAdjustments(user);
    expect(screen.getByRole('checkbox', { name: '用户临时文件' })).toBeVisible();
    expect(mock.scan).toHaveBeenCalledTimes(1);
    expect(mock.execute).not.toHaveBeenCalled();
  });
  it('does not scan on startup and has no system cleanup entry', async () => {
    render(<App />);
    await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled(), {
      timeout: 5000,
    });
    expect(document.querySelector('.scan-scope input[type="checkbox"]')).toBeNull();
    expect(screen.queryByRole('checkbox', { name: /系统临时文件/ })).not.toBeInTheDocument();
    expect(mock.scan).not.toHaveBeenCalled();
    expect(mock.execute).not.toHaveBeenCalled();
  });
  it('requires a separate permanent-delete confirmation', async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled(), {
      timeout: 5000,
    });
    await user.click(screen.getByRole('button', { name: '开始扫描' }));
    await user.click(await screen.findByRole('button', { name: /^清理已选/ }));
    expect(await screen.findByRole('alertdialog')).toHaveTextContent('不会进入回收站');
    expect(mock.execute).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: '取消' }));
    expect(mock.execute).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: /^清理已选/ }));
    await user.click(await screen.findByRole('button', { name: '永久删除' }));
    await waitFor(() => expect(mock.execute).toHaveBeenCalledTimes(1));
  });
  it('keeps a clear explanation after UAC refusal', async () => {
    mock.scan.mockResolvedValue({
      sessions: [{ scope: 'user', id: 'scan-1' }],
      warning: '系统清理授权被取消；仍可清理用户文件',
    });
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled(), {
      timeout: 5000,
    });
    await user.click(screen.getByRole('button', { name: '开始扫描' }));
    expect(await screen.findByText(/系统清理授权被取消/)).toBeVisible();
    expect(await screen.findByRole('button', { name: /^清理已选/ })).toBeEnabled();
  });
  it('has navigable history and excludes sensitive data from default categories', async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled(), {
      timeout: 5000,
    });
    await user.click(screen.getByRole('button', { name: '历史' }));
    expect(await screen.findByText('还没有清理记录')).toBeVisible();
    expect(
      rules
        .filter((r) => r.default_selected)
        .some((r) => /cookie|password|session|download/i.test(r.id)),
    ).toBe(false);
    expect(rules.some((r) => /cookie|password|session/i.test(r.id))).toBe(false);
  });
  it('locks mutations until a failed status query is reconciled', async () => {
    const readyJob = await mock.status();
    mock.status.mockRejectedValue(new Error('状态查询失败'));
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled(), {
      timeout: 5000,
    });
    await user.click(screen.getByRole('button', { name: '开始扫描' }));
    expect(await screen.findByText(/请重新检查，勿重复开始任务/)).toBeVisible();
    expect(screen.queryByRole('button', { name: '重新全面扫描' })).not.toBeInTheDocument();
    expect(mock.execute).not.toHaveBeenCalled();
    mock.status.mockResolvedValue(readyJob);
    await user.click(screen.getByRole('button', { name: '重新检查状态' }));
    await user.click(await screen.findByRole('button', { name: /^清理已选/ }));
    expect(mock.prepare.mock.calls[0][0]).toHaveLength(1);
    expect(mock.prepare.mock.calls[0][0][0].scope).toBe('user');
  });
  it('keeps the scan after changing appearance', async () => {
    const user = userEvent.setup();
    render(<App />);
    await startDefaultScan(user);
    await screen.findByRole('button', { name: /^清理已选/ });
    await user.click(screen.getByRole('button', { name: '设置' }));
    await user.click(screen.getByRole('button', { name: '深色' }));
    expect(await screen.findByText(/扫描结果与选择保持不变/)).toBeVisible();
    await user.click(screen.getByRole('button', { name: '清理' }));
    expect(screen.getByRole('button', { name: /^清理已选/ })).toBeEnabled();
    expect(mock.scan).toHaveBeenCalledTimes(1);
  });
  it('reconciles cleared analysis state after choosing an index location', async () => {
    const user = userEvent.setup();
    mock.pickAnalysisStorage.mockResolvedValue('F:\\DustAway-analysis-cache');
    render(<App />);
    await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled(), {
      timeout: 5000,
    });
    await user.click(screen.getByRole('button', { name: '设置' }));
    await user.click(screen.getByRole('button', { name: '帮助与关于' }));
    await user.click(screen.getByText('空间分析临时索引'));
    const before = mock.snapshot.mock.calls.length;
    await user.click(screen.getByRole('button', { name: '选择索引位置' }));
    expect(await screen.findByText('临时索引位置已更新，请重新开始空间分析。')).toBeVisible();
    expect(mock.snapshot.mock.calls.length).toBeGreaterThan(before);
    expect(screen.getByText('F:\\DustAway-analysis-cache')).toBeVisible();
    expect(mock.scan).not.toHaveBeenCalled();
  });
  it('subtracts page exclusions immediately and disables an empty plan', async () => {
    const user = userEvent.setup();
    render(<App />);
    await startDefaultScan(user);
    await openWorkbench(user);
    await user.click(await screen.findByRole('button', { name: '查看用户临时文件的文件' }));
    await waitFor(() => expect(screen.getByRole('button', { name: '取消本页选择' })).toBeEnabled());
    await user.click(screen.getByRole('button', { name: '取消本页选择' }));
    await user.click(screen.getByRole('button', { name: '关闭文件明细' }));
    expect(screen.getByRole('checkbox', { name: '用户临时文件' })).not.toBeChecked();
    expect(
      within(screen.getByLabelText('用户临时文件').closest('.rule-row') as HTMLElement).getByText(
        '未选择',
      ),
    ).toBeVisible();
    expect(screen.getByRole('checkbox', { name: '选择临时文件建议项' })).not.toBeChecked();
    expect(screen.queryByRole('button', { name: /^清理已选/ })).not.toBeInTheDocument();
    expect(mock.execute).not.toHaveBeenCalled();
  });
  it('rechecks browser availability without scanning again', async () => {
    const summary = await mock.summary();
    summary.rules[0].availability = 'browser_running';
    summary.rules[0].blocked = true;
    summary.rules[0].note = '浏览器仍在运行；关闭窗口后可能还有后台进程，请完全退出再检查';
    mock.summary.mockResolvedValue(summary);
    mock.refresh.mockImplementation(async () => {
      const updated = {
        ...summary,
        rules: [{ ...summary.rules[0], blocked: false, availability: null, note: null }],
      };
      // Subsequent polling must see the same backend state as the refresh response.
      mock.summary.mockResolvedValue(updated);
      return updated;
    });
    mock.refresh.mockRejectedValueOnce(new Error('运行状态暂时读取失败，请重试'));
    const user = userEvent.setup();
    render(<App />);
    await startDefaultScan(user);
    await openAdjustments(user);
    const recheck = await screen.findByRole('button', { name: '重新检查应用状态' });
    await waitFor(() => expect(recheck).toBeEnabled());
    expect(screen.queryByText(summary.rules[0].note)).not.toBeInTheDocument();
    expect(
      within(screen.getByRole('region', { name: '清理决策总览' })).getByText(/本次未清理 ·/),
    ).toBeVisible();
    await user.click(recheck);
    expect(await screen.findByText(/运行状态暂时读取失败，请重试/)).toBeVisible();
    expect(mock.scan).toHaveBeenCalledTimes(1);
    expect(mock.execute).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: '重新检查应用状态' }));
    expect(await screen.findByText(/原有候选和选择已保留/)).toBeVisible();
    expect(screen.queryByText(/运行状态暂时读取失败，请重试/)).not.toBeInTheDocument();
    await openAdjustments(user);
    await waitFor(() =>
      expect(screen.getByRole('checkbox', { name: '选择临时文件建议项' })).toBeEnabled(),
    );
    expect(screen.getByLabelText('用户临时文件')).not.toBeChecked();
    await user.click(screen.getByRole('checkbox', { name: '选择临时文件建议项' }));
    await waitFor(() => expect(screen.getByRole('button', { name: /^清理已选/ })).toBeEnabled());
    expect(mock.scan).toHaveBeenCalledTimes(1);
    expect(mock.execute).not.toHaveBeenCalled();
  });
});
it('formats IPC byte strings without losing integer precision', () => {
  expect(formatBytes('0')).toBe('0 B');
  expect(formatBytes('1024')).toBe('1.0 KB');
  expect(formatBytes('-2048')).toBe('−2.0 KB');
  expect(formatBytes('9007199254740993')).toBe('8.0 PB');
  expect(formatBytes('invalid')).toBe('—');
});

it('manual review selects one file only and cancelling its rule clears authorization', async () => {
  const boot = await mock.bootstrap();
  boot.rules = boot.rules.map((r: import('../types').Rule) =>
    r.id === 'windows.user_temp' ? { ...r, cleanup_policy: 'review' } : r,
  );
  mock.bootstrap.mockResolvedValue(boot);
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await openWorkbench(user);
  const detail = await screen.findByRole('button', { name: '查看用户临时文件的文件' });
  expect(screen.getByLabelText('用户临时文件')).not.toBeChecked();
  expect(screen.queryByRole('button', { name: /^清理已选/ })).not.toBeInTheDocument();
  await user.click(detail);
  const files = await within(await screen.findByRole('dialog')).findAllByRole('checkbox');
  expect(files).toHaveLength(3);
  files.forEach((file) => expect(file).not.toBeChecked());
  await user.click(files[1]);
  expect(files[0]).not.toBeChecked();
  expect(files[2]).not.toBeChecked();
  await user.click(screen.getByRole('button', { name: '关闭文件明细' }));
  expect(screen.getByLabelText('用户临时文件')).toBePartiallyChecked();
  await user.click(screen.getByRole('button', { name: /^清理已选/ }));
  const selection = mock.prepare.mock.calls[0][0][0].selection;
  expect(selection).toEqual({
    rule_ids: [],
    excluded_ids: [],
    included_ids: ['file-1'],
    reviewed_rule_ids: ['windows.user_temp'],
  });
  expect(screen.getByRole('alertdialog')).toHaveTextContent('你手动选择的需确认项目');
  await user.click(screen.getByRole('button', { name: '取消' }));
  await user.click(screen.getByLabelText('用户临时文件'));
  await user.click(screen.getByLabelText('用户临时文件'));
  expect(screen.queryByRole('button', { name: /^清理已选/ })).not.toBeInTheDocument();
  await user.click(detail);
  (await within(await screen.findByRole('dialog')).findAllByRole('checkbox')).forEach((file) =>
    expect(file).not.toBeChecked(),
  );
  expect(mock.execute).not.toHaveBeenCalled();
});

it('a new scan resets review selections and restores only recommended defaults', async () => {
  const boot = await mock.bootstrap();
  boot.rules = boot.rules.map((r: import('../types').Rule) =>
    r.id === 'windows.user_temp' ? { ...r, cleanup_policy: 'review' } : r,
  );
  mock.bootstrap.mockResolvedValue(boot);
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await waitFor(() => expect(screen.getByLabelText('用户临时文件')).toBeEnabled());
  await user.click(screen.getByLabelText('用户临时文件'));
  expect(screen.getByRole('button', { name: /^清理已选/ })).toBeEnabled();
  const summary = await mock.summary(),
    job = await mock.status();
  mock.scan.mockResolvedValue({ sessions: [{ scope: 'user', id: 'scan-2' }], warning: null });
  mock.summary.mockResolvedValue({ ...summary, session_id: 'scan-2' });
  mock.status.mockResolvedValue({ ...job, id: 'scan-2' });
  await user.click(screen.getByRole('button', { name: '重新全面扫描' }));
  await waitFor(() => expect(screen.getByLabelText('用户临时文件')).not.toBeChecked());
  expect(screen.queryByRole('button', { name: /^清理已选/ })).not.toBeInTheDocument();
  expect(mock.scan.mock.calls[1][0]).toEqual(boot.rules.map((r: import('../types').Rule) => r.id));
});

it('shows discovered size even when a category is unchecked or blocked', async () => {
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await screen.findByRole('button', { name: /^清理已选/ });
  const row = screen.getByLabelText('用户临时文件').closest('.rule-row')!;
  await user.click(screen.getByLabelText('用户临时文件'));
  expect(row).toHaveTextContent(formatBytes('3072'));
  expect(row).toHaveTextContent('3 个文件');
  expect(row).toHaveTextContent('未选择');
  expect(mock.execute).not.toHaveBeenCalled();
});
it('does not restore cleanup authorization from remembered scan categories', async () => {
  const boot = await mock.bootstrap();
  mock.bootstrap.mockResolvedValue({
    ...boot,
    preferences: {
      last_scan_rule_ids: ['edge.http', 'windows.system_temp', 'unknown', 'chrome.http'],
    },
    settings: { ...boot.settings, excluded_rules: ['chrome.http'] },
  });
  render(<App />);
  await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled(), {
    timeout: 5000,
  });
  expect(document.querySelector('.scan-scope input[type="checkbox"]')).toBeNull();
  expect(mock.scan).not.toHaveBeenCalled();
  expect(mock.execute).not.toHaveBeenCalled();
});
it('recovers an existing backend scan on reload with no restored selection or plan', async () => {
  mock.snapshot.mockImplementation(async () => ({
    status: await mock.status(),
    summary: await mock.summary(),
    report: null,
  }));
  render(<App />);
  expect(await screen.findByText(/已恢复扫描结果/)).toBeVisible();
  expect(screen.getByLabelText('用户临时文件')).not.toBeChecked();
  expect(screen.queryByRole('button', { name: /^清理已选/ })).not.toBeInTheDocument();
  expect(mock.scan).not.toHaveBeenCalled();
  expect(mock.prepare).not.toHaveBeenCalled();
});
it('reattaches a scan whose response was lost without sending it again', async () => {
  mock.scan.mockRejectedValue(new Error('response lost after acceptance'));
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  expect(await screen.findByText(/已重新接上后台任务/)).toBeVisible();
  expect(screen.getByRole('button', { name: /^清理已选/ })).toBeEnabled();
  expect(mock.scan).toHaveBeenCalledTimes(1);
});
it('does not replay execution after losing its response', async () => {
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await user.click(await screen.findByRole('button', { name: /^清理已选/ }));
  mock.execute.mockImplementation(async () => {
    const status = await mock.status();
    const summary = await mock.summary();
    mock.status.mockResolvedValue({ ...status, state: 'finished' });
    mock.summary.mockResolvedValue({ ...summary, state: 'finished' });
    mock.report.mockResolvedValue({
      id: 'plan-1',
      scope: 'user',
      status: 'finished',
      total: 3,
      deleted: 3,
      skipped: 0,
      failed: 0,
      unknown: 0,
      candidate_bytes: '3072',
      deleted_bytes: '3072',
      free_space_change: null,
      started_at: 0,
      finished_at: 1,
      categories: [],
    });
    throw new Error('execute reply lost');
  });
  await user.click(screen.getByRole('button', { name: '永久删除' }));
  expect(await screen.findByText('查看清理记录', { selector: 'summary' })).toBeVisible();
  expect(mock.execute).toHaveBeenCalledTimes(1);
  expect(mock.prepare).toHaveBeenCalledTimes(1);
});
it('keeps an exclusion draft across detail close and preserves it on a rejected save', async () => {
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await openWorkbench(user);
  await user.click(await screen.findByRole('button', { name: '查看用户临时文件的文件' }));
  await user.click(await screen.findByRole('button', { name: '取消本页选择' }));
  await user.click(screen.getByText('长期保留设置'));
  await user.click(screen.getByRole('button', { name: '将本类别已取消的 3 项加入长期排除' }));
  await user.click(screen.getByText('长期保留设置'));
  expect(screen.getByText(/3 项长期保留草稿尚未保存/)).toBeVisible();
  expect(screen.getByRole('button', { name: '核对并保存草稿' })).toBeVisible();
  await user.click(screen.getByRole('button', { name: '关闭文件明细' }));
  await user.click(screen.getByRole('button', { name: '重新全面扫描' }));
  expect(await screen.findByRole('alertdialog')).toHaveTextContent('未保存的排除草稿');
  await user.click(screen.getByRole('button', { name: '核对并保存' }));
  mock.addExclusions.mockRejectedValue(new Error('fixture write denied'));
  await user.click(screen.getByRole('button', { name: '保存排除并重新扫描' }));
  await waitFor(() => expect(mock.addExclusions).toHaveBeenCalledTimes(1));
  expect(mock.scan).toHaveBeenCalledTimes(1);
  expect(screen.getByRole('dialog')).toHaveTextContent('C:/fixture/0.tmp');
  expect(mock.addExclusions.mock.calls[0][2]).toEqual(
    expect.arrayContaining([{ candidate_id: 'file-0', kind: 'file' }]),
  );
});

it('keeps stopping after a lost cancel response until the backend confirms completion', async () => {
  const status = await mock.status(),
    summary = await mock.summary();
  mock.status.mockResolvedValue({ ...status, state: 'scanning' });
  mock.summary.mockResolvedValue({ ...summary, state: 'scanning' });
  mock.cancel.mockRejectedValue(new Error('cancel reply lost'));
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await user.click(await screen.findByRole('button', { name: '停止任务' }));
  expect(await screen.findByRole('button', { name: '停止中…' })).toBeDisabled();
  expect(mock.cancel).toHaveBeenCalledTimes(1);
  mock.status.mockResolvedValue({ ...status, state: 'cancelled' });
  mock.summary.mockResolvedValue({ ...summary, state: 'cancelled' });
  expect(
    await screen.findByRole('button', { name: '重新全面扫描' }, { timeout: 5000 }),
  ).toBeEnabled();
  expect(mock.execute).not.toHaveBeenCalled();
});
it('keeps saved exclusions when the following rescan cannot start', async () => {
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await openWorkbench(user);
  await user.click(await screen.findByRole('button', { name: '查看用户临时文件的文件' }));
  await user.click(await screen.findByRole('button', { name: '取消本页选择' }));
  await user.click(screen.getByText('长期保留设置'));
  await user.click(screen.getByRole('button', { name: '将本类别已取消的 3 项加入长期排除' }));
  await user.click(screen.getByRole('button', { name: '关闭文件明细' }));
  await user.click(screen.getByRole('button', { name: '核对排除草稿' }));
  mock.addExclusions.mockImplementation(async () => {
    mock.snapshot.mockResolvedValue({
      status: { id: '', state: 'idle', error: null },
      summary: { session_id: '', state: 'idle', rules: [] },
      report: null,
    });
    mock.scan.mockRejectedValue(new Error('fixture could not start'));
    return {
      settings: { theme: 'dark', exclusions: ['C:/fixture/0.tmp'], excluded_rules: [] },
      scan_invalidated: true,
    };
  });
  await user.click(screen.getByRole('button', { name: '保存排除并重新扫描' }));
  expect(await screen.findByText(/排除已保存，扫描未开始/)).toBeVisible();
  expect(screen.queryByRole('button', { name: /^清理已选/ })).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '设置' }));
  await user.click(screen.getByRole('button', { name: '保留清单' }));
  expect(screen.getByText('C:/fixture/0.tmp')).toBeVisible();
  expect(mock.execute).not.toHaveBeenCalled();
});

it('reloads startup data after bootstrap failure without starting a task', async () => {
  mock.bootstrap.mockRejectedValueOnce(new Error('startup unavailable'));
  const user = userEvent.setup();
  render(<App />);
  expect(await screen.findByText(/启动数据加载失败/)).toBeVisible();
  await user.click(screen.getByRole('button', { name: '重新加载' }));
  await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled(), {
    timeout: 5000,
  });
  expect(document.querySelector('.scan-scope input[type="checkbox"]')).toBeNull();
  expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled();
  expect(mock.scan).not.toHaveBeenCalled();
  expect(mock.execute).not.toHaveBeenCalled();
});
it('shows actual checked entries while a retained-only scan has no candidates', async () => {
  const status = await mock.status(),
    summary = await mock.summary();
  mock.status.mockResolvedValue({
    ...status,
    state: 'scanning',
    visited: 0,
    current_rule: 'windows.user_temp',
    scan_progress: { checked: 123, elapsed_ms: 15000, last_progress_ms: 1000 },
  });
  mock.summary.mockResolvedValue({
    ...summary,
    state: 'scanning',
    rules: [{ ...summary.rules[0], phase: 'scanning', files: 0, bytes: '0' }],
  });
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  expect(await screen.findByText(/已检查 123 项/)).toBeVisible();
  expect(screen.getByText('正在检查：临时文件 · 用户临时文件')).toBeVisible();
  expect(screen.getByText(/仍在等待当前检查返回/)).toBeVisible();
  expect(screen.queryByRole('button', { name: /^清理已选/ })).not.toBeInTheDocument();
});
it('preserves terminal results and separates unselected files from unfinished cleanup', async () => {
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await user.click(await screen.findByRole('button', { name: /^清理已选/ }));
  const status = await mock.status(),
    summary = await mock.summary();
  mock.execute.mockImplementation(async () => {
    mock.status.mockResolvedValue({ ...status, state: 'finished' });
    mock.summary.mockResolvedValue({ ...summary, state: 'finished' });
    mock.report.mockResolvedValue({
      id: 'plan-1',
      scope: 'user',
      status: 'finished',
      total: 2,
      deleted: 1,
      skipped: 1,
      failed: 0,
      unknown: 0,
      candidate_bytes: '2048',
      deleted_bytes: '1024',
      free_space_change: null,
      started_at: 0,
      finished_at: 1,
      categories: [
        {
          rule_id: 'windows.user_temp',
          total: 2,
          deleted: 1,
          skipped: 1,
          failed: 0,
          unknown: 0,
          deleted_bytes: '1024',
          reasons: [],
          retained: [],
        },
      ],
    });
    return { scopes: ['user'], warning: null };
  });
  await user.click(screen.getByRole('button', { name: '永久删除' }));
  expect(await screen.findByText(/清理已结束，保留、失败和未处理原因/)).toBeVisible();
  await user.click(screen.getByText('查看清理记录'));
  await user.click(screen.getByRole('button', { name: '查看临时文件清理结果' }));
  expect(
    within(screen.getByRole('region', { name: '各类别与未选择文件' })).getByText(
      '扫描时发现 3 个文件 · 3.0 KB',
    ),
  ).toBeVisible();
  await user.click(screen.getByRole('button', { name: '查看未纳入计划的 1 个文件' }));
  await waitFor(() =>
    expect(mock.candidates).toHaveBeenLastCalledWith(
      'user',
      'scan-1',
      'windows.user_temp',
      0,
      expect.objectContaining({ view: 'unplanned' }),
    ),
  );
  await user.click(screen.getByRole('button', { name: '关闭文件明细' }));
  mock.scan.mockRejectedValueOnce(new Error('not accepted'));
  await user.click(screen.getByRole('button', { name: '完成' }));
  await user.click(screen.getByRole('button', { name: '开始扫描' }));
  expect(await screen.findByText(/not accepted/)).toBeVisible();
  expect((await mock.report()).id).toBe('plan-1');
  expect(mock.execute).toHaveBeenCalledTimes(1);
});

it('does not show empty history when reading it failed', async () => {
  mock.history.mockRejectedValueOnce(new Error('history timeout')).mockResolvedValue([]);
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole('button', { name: '历史' }));
  expect(await screen.findByText(/历史读取失败/)).toBeVisible();
  expect(screen.queryByRole('button', { name: '前往清理' })).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '重试读取历史' }));
  await waitFor(() => expect(screen.queryByText(/历史读取失败/)).not.toBeInTheDocument());
  expect(mock.history).toHaveBeenCalledTimes(2);
});

it('viewing an analysis category preserves the range; only explicit add unions it for the next scan', async () => {
  const boot = await mock.bootstrap();
  boot.preferences.last_scan_rule_ids = ['windows.user_temp'];
  mock.bootstrap.mockResolvedValue(boot);
  const location = {
    token: 'volume',
    title: '系统盘',
    path: 'C:\\',
    kind: 'volume',
    is_system: true,
    supported: true,
  };
  const entry = {
    id: 'cache',
    parent: '0',
    name: 'Chrome 缓存',
    path: 'C:\\fixture',
    directory: true,
    bytes: '1024',
    files: 1,
    modified_unix: 0,
    shared: false,
    incomplete: false,
    issue: null,
    rule_ids: ['chrome.http'],
    excluded: false,
  };
  const initial = await mock.snapshot();
  mock.snapshot.mockResolvedValue({
    ...initial,
    analysis: {
      id: 'analysis',
      location,
      state: 'completed',
      started_at: 1,
      elapsed_ms: 1000,
      checked: 1,
      entries: 2,
      files: 1,
      logical_bytes: '1024',
      unique_bytes: '1024',
      issues: [],
      stale: false,
      error: null,
    },
  });
  mock.analysisLocations.mockResolvedValue([systemVolume, location]);
  mock.analysisContext.mockResolvedValue({
    location_token: 'volume',
    volume: 'C:\\',
    volume_key: 'id',
    available_bytes: '1024',
    sampled_at: 1,
    error: null,
  });
  mock.analysisChildren.mockResolvedValue({
    node: { ...entry, id: '0' },
    breadcrumbs: [],
    items: [entry],
    total: 1,
    offset: 0,
  });
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole('button', { name: /查找大文件/ }));
  await user.click(await screen.findByRole('button', { name: /查看详情：/ }));
  await user.click(screen.getByRole('button', { name: '查看对应清理类别' }));
  expect(screen.queryByRole('checkbox', { name: /Chrome 网页缓存/ })).not.toBeInTheDocument();
  expect(mock.scan).not.toHaveBeenCalled();
  expect(mock.prepare).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: '返回刚才的分析位置' }));
  const details = screen.getByRole('button', { name: /查看详情：/ });
  await waitFor(() => expect(details).toHaveFocus());
  await user.click(details);
  await user.click(screen.getByRole('button', { name: '返回列表' }));
  await user.click(screen.getByRole('button', { name: '清理' }));
  await user.click(screen.getByRole('button', { name: '返回刚才的分析位置' }));
  await waitFor(() =>
    expect(screen.getByRole('generic', { name: '本次范围空间排行' })).toHaveFocus(),
  );
  await user.click(screen.getByRole('button', { name: /查看详情：/ }));
  await user.click(screen.getByRole('button', { name: '查看对应清理类别' }));
  await user.click(screen.getByRole('button', { name: '准备重新全面扫描' }));
  expect(screen.queryByRole('checkbox', { name: /Chrome 网页缓存/ })).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '开始扫描' }));
  await waitFor(() =>
    expect(mock.scan).toHaveBeenCalledWith(
      rules.filter((r) => !r.requires_admin && true).map((r) => r.id),
      ['system-volume'],
    ),
  );
  expect(mock.execute).not.toHaveBeenCalled();
});

async function manualAnalysisFixture() {
  const location = { token: 'personal', title: '个人资料', path: 'G:\\fixture', supported: true };
  const entry = {
    id: 'folder',
    parent: '0',
    ancestors: ['0'],
    name: '过期资料',
    path: 'G:\\fixture\\folder',
    directory: true,
    bytes: '9007199254740993',
    files: 2,
    modified_unix: 0,
    shared: false,
    incomplete: false,
    issue: null,
    rule_ids: [],
    excluded: false,
    manual: { allowed: true, reason: null },
  };
  const initial = await mock.snapshot();
  mock.snapshot.mockResolvedValue({
    ...initial,
    analysis: {
      id: 'analysis-manual',
      location,
      state: 'completed',
      started_at: 1,
      elapsed_ms: 10,
      checked: 3,
      entries: 3,
      files: 2,
      logical_bytes: entry.bytes,
      unique_bytes: entry.bytes,
      issues: [],
      stale: false,
      error: null,
    },
  });
  mock.analysisLocations.mockResolvedValue([systemVolume, location]);
  mock.analysisContext.mockResolvedValue({
    location_token: 'personal',
    volume: 'G:\\',
    volume_key: 'fixture',
    available_bytes: '100000',
    sampled_at: 1,
    error: null,
  });
  mock.analysisChildren.mockResolvedValue({
    node: { ...entry, id: '0' },
    breadcrumbs: [],
    items: [entry],
    total: 1,
    offset: 0,
  });
  mock.prepare.mockResolvedValue([
    {
      scope: 'user',
      plan: {
        id: 'manual-plan',
        session_id: 'generated',
        files: 2,
        bytes: entry.bytes,
        rule_ids: [],
        impacts: [],
        categories: [],
        manual: {
          session: 'analysis-manual',
          files: 2,
          directories: 1,
          bytes: entry.bytes,
          retained: 1,
        },
      },
    },
  ]);
  return entry;
}

it('cleans directly from analysis and keeps choices, search, scroll and focus on cancel or stop', async () => {
  const entry = await manualAnalysisFixture();
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole('button', { name: /查找大文件/ }));
  const checkbox = await screen.findByRole('checkbox', { name: '选择过期资料' });
  expect(checkbox).not.toBeChecked();
  await user.click(checkbox);
  await user.type(screen.getByRole('searchbox'), '过期');
  await waitFor(() =>
    expect(mock.analysisDiscovery).toHaveBeenLastCalledWith(
      'analysis-manual',
      expect.objectContaining({ search: '过期' }),
    ),
  );
  const list = screen.getByLabelText('本次范围空间排行');
  list.scrollTop = 42;
  const trigger = screen.getByRole('button', { name: '清理已选' });
  expect(screen.getAllByRole('button', { name: '查看已选' })).toHaveLength(1);
  await user.click(trigger);
  expect(mock.prepare).toHaveBeenCalledWith(
    [],
    {
      session: 'analysis-manual',
      selected_ids: ['folder'],
      excluded_ids: [],
    },
    expect.any(String),
  );
  expect(
    screen.queryByText(/已恢复上次|已恢复扫描结果|请重新选择清理类别/),
  ).not.toBeInTheDocument();
  const firstPreparation = mock.prepare.mock.lastCall![2];
  const previewCalls = mock.manualPreview.mock.calls.length;
  const dialog = await screen.findByRole('alertdialog');
  expect(dialog).toHaveTextContent('2 个文件 · 1 个文件夹');
  expect(within(dialog).getByRole('region', { name: '确认自选范围' })).toHaveTextContent(
    '过期资料',
  );
  expect(dialog).toHaveTextContent(entry.path);
  expect(within(dialog).queryByText('查看类别与影响')).not.toBeInTheDocument();
  expect(dialog).toHaveTextContent('文件夹可能保留');
  expect(mock.execute).not.toHaveBeenCalled();
  await user.click(within(dialog).getByRole('button', { name: '取消' }));
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(document.querySelector('.app-shell')).toHaveAttribute('data-page', 'analysis');
  expect(screen.getByRole('searchbox')).toHaveValue('过期');
  expect(list.scrollTop).toBe(42);
  expect(checkbox).toBeChecked();
  const savedPlan = await mock.prepare();
  let finishPreparation!: (value: typeof savedPlan) => void;
  mock.prepare.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finishPreparation = resolve;
      }),
  );
  mock.cancelPreparation.mockResolvedValue(undefined);
  await user.dblClick(screen.getByRole('button', { name: /^清理已选/ }));
  await user.click(await screen.findByRole('button', { name: '停止准备' }));
  expect(mock.cancelPreparation).toHaveBeenCalledWith(mock.prepare.mock.lastCall![2]);
  expect(mock.prepare.mock.lastCall![2]).not.toBe(firstPreparation);
  expect(screen.getByRole('button', { name: '正在停止准备' })).toBeDisabled();
  expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  await act(async () => finishPreparation(savedPlan));
  await waitFor(() => expect(screen.getByRole('button', { name: /^清理已选/ })).toBeEnabled());
  expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  expect(mock.manualPreview.mock.calls.length).toBe(previewCalls);
  expect(mock.prepare).toHaveBeenCalledTimes(3);
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(screen.getByRole('searchbox')).toHaveValue('过期');
  expect(list.scrollTop).toBe(42);
  expect(checkbox).toBeChecked();
  await user.click(screen.getByRole('button', { name: /^清理已选/ }));
  await user.click(await screen.findByRole('button', { name: '永久删除' }));
  await waitFor(() => expect(mock.execute).toHaveBeenCalledTimes(1));
  expect(document.querySelector('.app-shell')).toHaveAttribute('data-page', 'clean');
  expect(mock.scan).not.toHaveBeenCalled();
});

it.each(['pending', 'failed', 'empty-directory'] as const)(
  'allows direct final verification with %s manual statistics',
  async (state) => {
    await manualAnalysisFixture();
    if (state === 'pending') mock.manualPreview.mockImplementation(() => new Promise(() => {}));
    if (state === 'failed') mock.manualPreview.mockRejectedValue(new Error('统计暂不可用'));
    if (state === 'empty-directory') {
      mock.manualPreview.mockImplementation(async (selection, request_id) => ({
        request_id,
        session: selection.session,
        files: 0,
        directories: 1,
        bytes: '0',
        retained: 0,
      }));
      const [item] = await mock.prepare();
      mock.prepare.mockResolvedValue([
        {
          ...item,
          plan: {
            ...item.plan,
            files: 0,
            bytes: '0',
            manual: { ...item.plan.manual, files: 0, directories: 1, bytes: '0' },
          },
        },
      ]);
      mock.prepare.mockClear();
    }
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole('button', { name: /查找大文件/ }));
    await user.click(await screen.findByRole('checkbox', { name: '选择过期资料' }));
    const dock = document.querySelector('.task-dock') as HTMLElement;
    if (state === 'failed') await within(dock).findByText(/总量暂不可用/);
    else if (state === 'empty-directory') await within(dock).findByText(/0 个文件 · 1 个文件夹/);
    else await waitFor(() => expect(mock.manualPreview).toHaveBeenCalled());
    const primary = within(dock).getByRole('button', { name: '清理已选' });
    expect(primary).toBeEnabled();
    await user.click(primary);
    const dialog = await screen.findByRole('alertdialog');
    expect(within(dialog).getByRole('button', { name: '永久删除' })).toBeEnabled();
    if (state === 'empty-directory') expect(dialog).toHaveTextContent('0 个文件 · 1 个文件夹');
    expect(mock.prepare).toHaveBeenCalledTimes(1);
    expect(document.querySelector('.app-shell')).toHaveAttribute('data-page', 'analysis');
    await user.click(within(dialog).getByRole('button', { name: '取消' }));
    expect(mock.execute).not.toHaveBeenCalled();
  },
);

it.each(['empty', 'error'] as const)(
  'keeps the analysis in place after %s verification',
  async (result) => {
    await manualAnalysisFixture();
    if (result === 'empty') mock.prepare.mockResolvedValue([]);
    else mock.prepare.mockRejectedValue(new Error('夹具核验失败'));
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole('button', { name: /查找大文件/ }));
    await user.click(await screen.findByRole('checkbox', { name: '选择过期资料' }));
    await user.type(screen.getByRole('searchbox'), '过期');
    const primary = screen.getByRole('button', { name: '清理已选' });
    await user.click(primary);
    await screen.findByText(result === 'empty' ? /核验后没有可删除的内容/ : /夹具核验失败/);
    await waitFor(() => expect(primary).toHaveFocus());
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    expect(screen.getByRole('searchbox')).toHaveValue('过期');
    expect(screen.getByRole('checkbox', { name: '选择过期资料' })).toBeChecked();
    expect(document.querySelector('.app-shell')).toHaveAttribute('data-page', 'analysis');
    expect(mock.execute).not.toHaveBeenCalled();
  },
);

it('shows the same mixed source selection in the analysis dock, optional cart and confirmation', async () => {
  const entry = await manualAnalysisFixture();
  const original = await mock.snapshot();
  mock.snapshot.mockImplementation(async () =>
    mock.scan.mock.calls.length
      ? { ...original, status: await mock.status(), summary: await mock.summary() }
      : original,
  );
  const [item] = await mock.prepare();
  mock.prepare.mockResolvedValue([
    {
      ...item,
      plan: {
        ...item.plan,
        files: 5,
        bytes: String(BigInt(entry.bytes) + 3072n),
        rule_ids: ['windows.user_temp'],
        categories: [{ rule_id: 'windows.user_temp', files: 3, bytes: '3072' }],
      },
    },
  ]);
  mock.prepare.mockClear();
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await user.click(screen.getByRole('button', { name: '空间分析' }));
  await user.click(await screen.findByRole('checkbox', { name: '选择过期资料' }));
  const dock = document.querySelector('.task-dock') as HTMLElement;
  expect(within(dock).getByText('包含垃圾扫描中已选的项目')).toBeVisible();
  expect(within(dock).getByText('包含垃圾扫描中已选的项目')).toHaveAttribute(
    'data-important',
    'true',
  );
  await within(dock).findByText(/5 个文件 · 1 个文件夹/);
  await user.click(screen.getByRole('button', { name: '查看已选' }));
  expect(await screen.findByRole('region', { name: '本次自选清单' })).toHaveTextContent(entry.name);
  await user.click(screen.getByRole('button', { name: '空间分析' }));
  expect(screen.getByRole('checkbox', { name: '选择过期资料' })).toBeChecked();
  await user.click(screen.getByRole('button', { name: '清理已选' }));
  const dialog = await screen.findByRole('alertdialog');
  expect(dialog).toHaveTextContent('5 个文件 · 1 个文件夹');
  expect(dialog).toHaveTextContent('垃圾扫描已选：3 个文件');
  expect(dialog).toHaveTextContent(entry.name);
  expect(mock.prepare).toHaveBeenCalledWith(
    [
      expect.objectContaining({
        selection: expect.objectContaining({ rule_ids: ['windows.user_temp'] }),
      }),
    ],
    { session: 'analysis-manual', selected_ids: ['folder'], excluded_ids: [] },
    expect.any(String),
  );
  await user.click(within(dialog).getByRole('button', { name: '取消' }));
  expect(mock.execute).not.toHaveBeenCalled();
});

it('returns home from an empty scan without changing the selected disk or starting another scan', async () => {
  const empty = await mock.summary();
  mock.summary.mockResolvedValue({
    ...empty,
    rules: empty.rules.map((row: (typeof empty.rules)[number]) => ({
      ...row,
      files: 0,
      bytes: '0',
    })),
  });
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await user.click(await screen.findByRole('button', { name: '返回首页' }));
  expect(screen.getByRole('button', { name: '开始扫描' })).toBeVisible();
  expect(screen.getByRole('checkbox', { name: '扫描 C:\\' })).toBeChecked();
  expect(mock.scan).toHaveBeenCalledTimes(1);
  expect(mock.prepare).not.toHaveBeenCalled();
});

it('invalidates only the rescanned source and clears frozen plans on selection changes', async () => {
  const original = await mock.snapshot();
  const analysis = {
    id: 'manual-source',
    location: { token: 'personal', title: '资料', path: 'G:/fixture' },
    state: 'completed',
    started_at: 1,
    elapsed_ms: 1,
    checked: 1,
    entries: 1,
    files: 1,
    logical_bytes: '1',
    unique_bytes: '1',
    issues: [],
    stale: false,
    error: null,
  };
  mock.snapshot.mockResolvedValue({ ...original, analysis });
  const hook = renderHook(() => useAppController());
  await waitFor(() => expect(hook.result.current.busy).toBe(false));
  const file = {
    id: 'item',
    parent: null,
    ancestors: [],
    name: '资料',
    path: 'G:/fixture/item',
    directory: false,
    bytes: '1',
    files: 1,
    modified_unix: 0,
    shared: false,
    incomplete: false,
    issue: null,
    rule_ids: [],
    excluded: false,
    manual: { allowed: true, reason: null },
  };
  act(() => hook.result.current.selectManual(file, true));
  act(() => hook.result.current.setSelected(new Set(['windows.user_temp'])));
  const plans = await mock.prepare();
  act(() => hook.result.current.setPlans(plans));
  expect(hook.result.current.plans.length).toBeGreaterThan(0);
  act(() => hook.result.current.selectManual(file, false));
  expect(hook.result.current.plans).toEqual([]);
  act(() => hook.result.current.selectManual(file, true));
  const summary = { ...(await mock.summary()), session_id: 'garbage-replacement' };
  const next = {
    ...original,
    status: { ...original.status, id: 'garbage-replacement', state: 'ready' },
    summary,
    analysis,
  };
  mock.snapshot.mockResolvedValue(next);
  await act(() => hook.result.current.reconcile(['user']));
  expect(hook.result.current.selected.size).toBe(0);
  expect(hook.result.current.hasManual).toBe(true);
  act(() => hook.result.current.setSelected(new Set(['windows.user_temp'])));
  mock.snapshot.mockResolvedValue({ ...next, analysis: { ...analysis, id: 'new-analysis' } });
  await act(() => hook.result.current.reconcile(['user']));
  expect(hook.result.current.hasManual).toBe(false);
  expect(hook.result.current.selected.has('windows.user_temp')).toBe(true);
  act(() => hook.result.current.selectManual(file, true));
  await waitFor(() => expect(hook.result.current.manualPreview.data).not.toBeNull());
  mock.save.mockResolvedValueOnce({
    settings: { theme: 'system', exclusions: [file.path], excluded_rules: [] },
    scan_invalidated: true,
  });
  await act(() => hook.result.current.save({ exclusions: [file.path] }));
  expect(hook.result.current.analysis?.stale).toBe(true);
  expect(hook.result.current.hasManual).toBe(false);
  expect(hook.result.current.manualPreview.data).toBeNull();
});
it('keeps homepage C readings independent of the analysis directory and supports switching to D', async () => {
  const d = { ...systemVolume, token: 'data-volume', path: 'D:\\', is_system: false };
  mock.analysisLocations.mockResolvedValue([systemVolume, d]);
  mock.analysisContext.mockImplementation(async (token) => ({
    location_token: token,
    volume: token === 'data-volume' ? 'D:\\' : 'C:\\',
    volume_key: token,
    available_bytes: token === 'data-volume' ? '2000' : '1000',
    sampled_at: 1,
    error: null,
  }));
  const hook = renderHook(() => useAppController());
  await waitFor(() =>
    expect(
      hook.result.current.homeSpace.readings.get('system-volume')?.context?.available_bytes,
    ).toBe('1000'),
  );
  act(() => {
    hook.result.current.setTab('analysis');
    hook.result.current.journey.setSelected('data-volume');
  });
  await waitFor(() =>
    expect(hook.result.current.journey.context?.location_token).toBe('data-volume'),
  );
  expect(hook.result.current.homeSpace.volumes.map((v) => v.token)).toEqual(['system-volume']);
  expect(
    hook.result.current.homeSpace.readings.get('system-volume')?.context?.available_bytes,
  ).toBe('1000');
  act(() => hook.result.current.setScanVolumeSelection(['data-volume']));
  await waitFor(() =>
    expect(
      hook.result.current.homeSpace.readings.get('data-volume')?.context?.available_bytes,
    ).toBe('2000'),
  );
  expect(hook.result.current.homeSpace.volumes.map((v) => v.token)).toEqual(['data-volume']);
});

it('finishes viewing without rescanning, deleting reports or replaying cleanup', async () => {
  await restoreReport([
    outcome({ failed: 0, skipped: 1, reasons: [{ code: 'protected', count: 1 }] }),
  ]);
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole('button', { name: '完成' }));
  expect(await screen.findByRole('button', { name: '开始扫描' })).toBeEnabled();
  expect(screen.queryByRole('button', { name: '完成' })).not.toBeInTheDocument();
  expect((await mock.report()).id).toBe('result-1');
  await user.click(screen.getByRole('button', { name: '历史' }));
  await user.click(screen.getByRole('button', { name: '清理' }));
  expect(screen.getByRole('button', { name: '开始扫描' })).toBeVisible();
  expect(screen.queryByRole('button', { name: '查看扫描范围' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '查看全部检查范围' })).not.toBeInTheDocument();
  expect(mock.scan).not.toHaveBeenCalled();
  expect(mock.prepare).not.toHaveBeenCalled();
  expect(mock.execute).not.toHaveBeenCalled();
});
it('keeps an empty verified plan out of permanent-delete confirmation', async () => {
  mock.prepare.mockResolvedValue([]);
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await user.click(await screen.findByRole('button', { name: /^清理已选/ }));
  expect(await screen.findByText(/核验后没有可删除的内容/)).toBeVisible();
  expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  expect(screen.getByRole('checkbox', { name: '选择临时文件建议项' })).toBeChecked();
  expect(mock.execute).not.toHaveBeenCalled();
});
it('explains reduced verified scope in the same confirmation and keeps choices on cancel', async () => {
  const [prepared] = await mock.prepare();
  mock.prepare.mockResolvedValue([
    {
      ...prepared,
      plan: {
        ...prepared.plan,
        files: 2,
        bytes: '2048',
        categories: [{ rule_id: 'windows.user_temp', files: 2, bytes: '2048' }],
      },
    },
  ]);
  const user = userEvent.setup();
  render(<App />);
  await startDefaultScan(user);
  await user.click(await screen.findByRole('button', { name: /^清理已选/ }));
  const dialog = await screen.findByRole('alertdialog');
  expect(dialog).toHaveTextContent('核验后有 1 项已保留');
  expect(dialog.querySelector('.confirm-total')).toHaveTextContent('2.0 KB2 个文件 · 0 个文件夹');
  expect(screen.getAllByRole('alertdialog')).toHaveLength(1);
  await user.click(within(dialog).getByRole('button', { name: '取消' }));
  expect(screen.getByRole('checkbox', { name: '选择临时文件建议项' })).toBeChecked();
  expect(mock.execute).not.toHaveBeenCalled();
});
it('bulk purpose selection is policy scoped and preserves unrelated file overrides', async () => {
  const boot = await mock.bootstrap(),
    summary = await mock.summary();
  boot.rules = boot.rules.map((rule: import('../types').Rule) =>
    rule.id === 'windows.user_temp' ? { ...rule, cleanup_policy: 'review' } : rule,
  );
  mock.bootstrap.mockResolvedValue(boot);
  mock.summary.mockResolvedValue({
    ...summary,
    rules: ['windows.user_temp', 'edge.http', 'weixin.logs'].map((rule_id) => ({
      ...summary.rules[0],
      rule_id,
    })),
  });
  const { result } = renderHook(() => useAppController());
  await waitFor(() =>
    expect(result.current.canScanVolumes && !result.current.initializing).toBe(true),
  );
  await act(() => result.current.scan());
  await waitFor(() => expect(result.current.selected.has('edge.http')).toBe(true));
  act(() =>
    result.current.setExcluded(
      new Map([
        ['edge-file', { scope: 'user', session: 'scan-1', rule_id: 'edge.http', bytes: '1024' }],
      ]),
    ),
  );
  act(() =>
    result.current.selectRuleGroup(
      ['windows.user_temp', 'edge.http', 'weixin.logs'],
      'review',
      true,
    ),
  );
  expect(result.current.selected.has('windows.user_temp')).toBe(true);
  expect(result.current.reviewed.has('windows.user_temp')).toBe(true);
  expect(result.current.selected.has('weixin.logs')).toBe(false);
  expect(result.current.excluded.has('edge-file')).toBe(true);
  act(() =>
    result.current.selectRuleGroup(['windows.user_temp', 'edge.http'], 'recommended', false),
  );
  expect(result.current.selected.has('edge.http')).toBe(false);
  expect(result.current.excluded.has('edge-file')).toBe(false);
  expect(result.current.selected.has('windows.user_temp')).toBe(true);
  expect(result.current.reviewed.has('windows.user_temp')).toBe(true);
  act(() => result.current.selectRuleGroup(['windows.user_temp'], 'review', false));
  expect(result.current.reviewed.has('windows.user_temp')).toBe(false);
  expect(result.current.selected.has('windows.user_temp')).toBe(false);
  expect(mock.prepare).not.toHaveBeenCalled();
});

it('automatically refreshes only a terminal tool rule once and preserves other selections and detail', async () => {
  const hook = renderHook(() => useAppController());
  await waitFor(() => expect(hook.result.current.initializing).toBe(false));
  await waitFor(() => expect(hook.result.current.canScanVolumes).toBe(true));
  await act(() => hook.result.current.startScan(['windows.user_temp', 'dev.uv']));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  const original = await mock.summary();
  const row = {
    ...original.rules[0],
    rule_id: 'dev.uv',
    revision: 1,
    stale: true,
    pending_tool_task: 'uv-terminal',
    tool: { ready: true },
  };
  let next = { ...original, rules: [...original.rules, row] };
  mock.summary.mockImplementation(async () => next);
  const detail = rules.find((r) => r.id === 'windows.user_temp')!;
  act(() => {
    hook.result.current.setSelected(new Set(['windows.user_temp']));
    hook.result.current.openDetail(detail);
    hook.result.current.setExpanded(new Set(['windows', 'developer']));
  });
  mock.refreshRule.mockImplementation(async (session, rule) => {
    expect(session).toBe('scan-1');
    expect(rule).toBe('dev.uv');
    next = {
      ...next,
      rules: [
        original.rules[0],
        { ...row, revision: 2, stale: false, pending_tool_task: null, files: 0, bytes: '0' },
      ],
    };
    return 'refresh-one';
  });
  mock.uvTask.mockResolvedValue({
    id: 'uv-terminal',
    session: 'scan-1',
    status: 'completed',
    finished_at: 2,
  });
  await act(() => hook.result.current.uv.refresh());
  await waitFor(() => expect(mock.refreshRule).toHaveBeenCalledTimes(1));
  await waitFor(() =>
    expect(hook.result.current.summaryRows.find((r) => r.rule_id === 'dev.uv')?.revision).toBe(2),
  );
  expect(hook.result.current.selected.has('windows.user_temp')).toBe(true);
  expect(hook.result.current.detail?.id).toBe('windows.user_temp');
  expect(hook.result.current.expanded.has('developer')).toBe(true);
  expect(hook.result.current.uv.stale).toBe(false);
  await act(() => hook.result.current.uv.refresh());
  await act(() => hook.result.current.reconcile());
  expect(mock.refreshRule).toHaveBeenCalledTimes(1);
  expect(mock.scan).toHaveBeenCalledTimes(1);
});

it('queries a failed refresh submission without replay and cancels only the displayed refresh task', async () => {
  const hook = renderHook(() => useAppController());
  await waitFor(() => expect(hook.result.current.initializing).toBe(false));
  await waitFor(() => expect(hook.result.current.canScanVolumes).toBe(true));
  await act(() => hook.result.current.startScan(['windows.user_temp', 'dev.uv']));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  const status = await mock.status();
  const refresh = {
    id: 'refresh-live',
    rule_id: 'dev.uv',
    revision: 1,
    state: 'scanning',
    progress: { checked: 1, elapsed_ms: 1, last_progress_ms: 1 },
    error: null,
  };
  mock.refreshRule.mockImplementation(async () => {
    mock.status.mockResolvedValue({ ...status, rule_refresh: refresh });
    throw new Error('response lost');
  });
  await act(() => hook.result.current.refreshRule('dev.uv'));
  expect(mock.refreshRule).toHaveBeenCalledTimes(1);
  expect(hook.result.current.busy).toBe(true);
  expect(hook.result.current.currentJob).toBeUndefined();
  mock.cancel.mockImplementation(async () =>
    mock.status.mockResolvedValue({
      ...status,
      rule_refresh: { ...refresh, state: 'cancelled', error: '检查已取消；旧结果已保留' },
    }),
  );
  await act(() => hook.result.current.cancelRuleRefresh());
  expect(mock.cancel).toHaveBeenCalledWith([{ scope: 'user', id: 'scan-1' }], 'refresh-live');
  await waitFor(() => expect(hook.result.current.busy).toBe(false));
  expect(hook.result.current.selected.has('windows.user_temp')).toBe(true);
  expect(mock.refreshRule).toHaveBeenCalledTimes(1);
});
