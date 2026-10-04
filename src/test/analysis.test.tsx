import { useState } from 'react';
import { toggleManual, toggleManualBatch, type ManualChoices } from '../manualSelection';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { AnalysisPanel } from '../AnalysisPanel';
import type { AnalysisEntry, AnalysisSnapshot } from '../types';
const mock = vi.hoisted(() => ({
  analysisLocations: vi.fn(),
  analysisContext: vi.fn(),
  analysisFiles: vi.fn(),
  analysisDiscovery: vi.fn(),
  apps: vi.fn(),
  storage: vi.fn(),
  analysisChildren: vi.fn(),
  revealAnalysis: vi.fn(),
  pickAnalysis: vi.fn(),
  refineAnalysis: vi.fn(),
  analysisRead: vi.fn(),
  analysisCount: vi.fn(),
  cancelAnalysisQuery: vi.fn(),
}));
vi.mock('../api', () => ({ desktop: true, api: mock }));
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 126,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({ index, start: index * 126, size: 126 })),
    measureElement: () => {},
    measure: () => {},
    scrollToIndex: vi.fn(),
  }),
}));
const location = { token: 'root-token', title: '当前用户文件夹', path: 'C:\\fixture' };
const node: AnalysisEntry = {
  id: '0',
  parent: null,
  name: 'fixture',
  path: location.path,
  directory: true,
  bytes: '1024',
  files: 1,
  modified_unix: 0,
  shared: false,
  incomplete: false,
  issue: null,
  rule_ids: [],
  excluded: false,
};
const snapshot: AnalysisSnapshot = {
  id: 'session',
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
};
const props = {
  active: true,
  busy: false,
  stopping: false,
  onStart: vi.fn(),
  onStop: vi.fn(),
  onCleanup: vi.fn(),
};
beforeEach(() => {
  vi.clearAllMocks();
  mock.cancelAnalysisQuery.mockResolvedValue(undefined);
  mock.analysisContext.mockResolvedValue({
    location_token: location.token,
    volume: 'C:\\',
    available_bytes: '9007199254740993',
    sampled_at: 1,
    error: null,
  });
  mock.analysisDiscovery.mockResolvedValue({ items: [], total: 0, offset: 0 });
  mock.analysisFiles.mockResolvedValue({ items: [], total: 0, offset: 0 });
  mock.analysisLocations.mockResolvedValue([location]);
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items: [],
    total: 0,
    offset: 0,
  });
});
afterEach(cleanup);

it('uses allocation metrics, keyset pagination and cancels superseded disk queries', async () => {
  mock.analysisRead.mockImplementation((q) =>
    Promise.resolve({
      request_id: q.request_id,
      generated_at: 1,
      items: [
        {
          ...node,
          id: `file-${q.offset}`,
          directory: false,
          name: `结果 ${q.offset}`,
          allocated_bytes: '4096',
        },
      ],
      total: 401,
      offset: q.offset,
      next_cursor: q.offset === 0 ? 'next-page' : null,
      node: null,
      breadcrumbs: [],
      coverage: { completed: 0, incomplete: 0, pending: 0 },
    }),
  );
  const user = userEvent.setup();
  const view = render(
    <AnalysisPanel
      {...props}
      snapshot={{ ...snapshot, query_api_version: 2, allocated_bytes: '4096' }}
    />,
  );
  await screen.findByText('结果 0');
  const first = mock.analysisRead.mock.calls.at(-1)![0];
  expect(first.metric).toBe('allocated');
  await user.click(screen.getByRole('button', { name: '下一页' }));
  await screen.findByText('结果 200');
  expect(mock.analysisRead).toHaveBeenLastCalledWith(
    expect.objectContaining({ cursor: 'next-page', offset: 200 }),
  );
  expect(mock.cancelAnalysisQuery).toHaveBeenCalledWith(first.request_id);
  await user.selectOptions(screen.getByLabelText('统计口径'), 'logical');
  await waitFor(() =>
    expect(mock.analysisRead).toHaveBeenLastCalledWith(
      expect.objectContaining({ metric: 'logical', cursor: null, offset: 0 }),
    ),
  );
  const last = mock.analysisRead.mock.calls.at(-1)![0];
  view.unmount();
  expect(mock.cancelAnalysisQuery).toHaveBeenCalledWith(last.request_id);
  expect(mock.analysisDiscovery).not.toHaveBeenCalled();
});

it('keeps a readable first page when asynchronous counting fails', async () => {
  mock.analysisRead.mockImplementation((q) =>
    Promise.resolve({
      request_id: q.request_id,
      items: [{ ...node, id: 'file', directory: false, name: '已读取文件', allocated_bytes: null }],
      total: null,
      offset: 0,
      next_cursor: 'next',
      node: null,
      breadcrumbs: [],
    }),
  );
  mock.analysisCount.mockResolvedValue({
    total: null,
    finished: true,
    error: '统计未完成，请刷新结果',
  });
  render(<AnalysisPanel {...props} snapshot={{ ...snapshot, query_api_version: 2 }} />);
  await screen.findByText('已读取文件');
  expect(await screen.findByText(/^总数未完成/)).toBeVisible();
  expect(screen.queryByText('总数统计中')).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '下一页' })).toBeEnabled();
  expect(screen.getByText('占用未知')).toBeVisible();
});

async function openFilters(user: ReturnType<typeof userEvent.setup>) {
  const summary = screen.getByText(/^筛选(?: · \d+ 项已应用)?$/);
  if (!summary.closest('details')?.open) await user.click(summary);
}

it('discovers across the session and restores filters, page, scroll and focus after entering a directory', async () => {
  const child = {
    ...node,
    id: 'dep',
    parent: '0',
    name: 'node_modules',
    path: 'C:\\fixture\\project\\node_modules',
    bytes: '9007199254740993',
  };
  mock.analysisDiscovery.mockImplementation((_session, q) =>
    Promise.resolve({ items: [child], total: 401, offset: q.offset }),
  );
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await user.click(await screen.findByRole('button', { name: '大目录' }));
  await openFilters(user);
  await user.selectOptions(screen.getByLabelText('用途筛选'), 'development');
  await user.type(screen.getByRole('searchbox'), 'node');
  await waitFor(() =>
    expect(mock.analysisDiscovery).toHaveBeenLastCalledWith(
      'session',
      expect.objectContaining({ search: 'node', category: 'development', kind: 'directory' }),
    ),
  );
  await user.click(screen.getByRole('button', { name: '下一页' }));
  await waitFor(() =>
    expect(mock.analysisDiscovery).toHaveBeenLastCalledWith(
      'session',
      expect.objectContaining({ offset: 200 }),
    ),
  );
  const list = screen.getByRole('generic', { name: '本次范围空间排行' });
  list.scrollTop = 84;
  await user.click(screen.getByRole('button', { name: 'node_modules' }));
  await waitFor(() =>
    expect(mock.analysisChildren).toHaveBeenLastCalledWith('session', 'dep', '', 0, true),
  );
  await user.click(screen.getByRole('button', { name: '返回空间排行' }));
  await waitFor(() => expect(screen.getByRole('button', { name: 'node_modules' })).toHaveFocus());
  expect(screen.getByRole('searchbox')).toHaveValue('node');
  expect(screen.getByLabelText('用途筛选')).toHaveValue('development');
  expect(screen.getByRole('generic', { name: '本次范围空间排行' }).scrollTop).toBe(84);
  expect(mock.analysisDiscovery).toHaveBeenLastCalledWith(
    'session',
    expect.objectContaining({ offset: 200 }),
  );
  expect(props.onStart).not.toHaveBeenCalled();
  expect(props.onCleanup).not.toHaveBeenCalled();
});

it('keeps discovery filters through review, page hiding and cleanup handoff without changing selection', async () => {
  const child = { ...node, id: 'cache', parent: '0', name: '缓存', rule_ids: ['chrome.http'] };
  mock.analysisDiscovery.mockResolvedValue({ items: [child], total: 1, offset: 0 });
  const user = userEvent.setup();
  const rendered = render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await user.click(await screen.findByRole('button', { name: '大文件' }));
  await openFilters(user);
  await user.selectOptions(screen.getByLabelText('用途筛选'), 'applications');
  await user.click(await screen.findByRole('button', { name: /查看详情：/ }));
  await user.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: '查看对应清理类别' }),
  );
  expect(props.onCleanup).toHaveBeenCalledWith(['chrome.http']);
  rendered.rerender(<AnalysisPanel {...props} active={false} snapshot={snapshot} />);
  rendered.rerender(<AnalysisPanel {...props} snapshot={snapshot} />);
  expect(screen.getByLabelText('用途筛选')).toHaveValue('applications');
  await user.click(screen.getAllByRole('button', { name: /查看详情：/ })[0]);
  await user.click(screen.getByRole('button', { name: '关闭文件详情' }));
  await waitFor(() => expect(screen.getByRole('button', { name: /查看详情：/ })).toHaveFocus());
  expect(props.onStart).not.toHaveBeenCalled();
});

it('opens incomplete directories without leftover restrictions and offers smaller scope when none are known', async () => {
  const user = userEvent.setup();
  render(
    <AnalysisPanel
      {...props}
      snapshot={{ ...snapshot, state: 'partial', issues: [{ code: 'entry_limit', count: 1 }] }}
    />,
  );
  await user.click(await screen.findByRole('button', { name: '大文件' }));
  await openFilters(user);
  await user.selectOptions(screen.getByLabelText('用途筛选'), 'ai');
  await user.type(screen.getByRole('searchbox'), 'model');
  await user.click(screen.getByRole('button', { name: '查看待补充目录' }));
  await waitFor(() =>
    expect(mock.analysisDiscovery).toHaveBeenLastCalledWith(
      'session',
      expect.objectContaining({
        category: 'all',
        search: '',
        min_bytes: '0',
        include_nested: true,
        incomplete_only: true,
        offset: 0,
      }),
    ),
  );
  await user.click(await screen.findByRole('button', { name: '选择较小范围' }));
  expect(screen.getByRole('button', { name: '选择目录' })).toHaveFocus();
  expect(props.onStart).not.toHaveBeenCalled();
});

it('rejects discovery while scanning and resets discovery after a new accepted analysis', async () => {
  const user = userEvent.setup();
  const rendered = render(
    <AnalysisPanel {...props} snapshot={{ ...snapshot, state: 'scanning' }} />,
  );
  expect(await screen.findByRole('button', { name: '大文件' })).toBeDisabled();
  expect(mock.analysisDiscovery).not.toHaveBeenCalled();
  rendered.rerender(<AnalysisPanel {...props} snapshot={snapshot} />);
  await user.click(screen.getByRole('button', { name: '大文件' }));
  await user.click(screen.getByRole('button', { name: '大文件' }));
  await openFilters(user);
  await user.selectOptions(screen.getByLabelText('用途筛选'), 'unclassified');
  rendered.rerender(
    <AnalysisPanel {...props} snapshot={{ ...snapshot, id: 'new', state: 'scanning' }} />,
  );
  expect(screen.getByRole('button', { name: '目录浏览' })).toHaveAttribute('aria-pressed', 'true');
  rendered.rerender(<AnalysisPanel {...props} snapshot={{ ...snapshot, id: 'new' }} />);
  await user.click(screen.getByRole('button', { name: '大文件' }));
  await waitFor(() =>
    expect(mock.analysisDiscovery).toHaveBeenLastCalledWith(
      'new',
      expect.objectContaining({ kind: 'file', category: 'all', offset: 0 }),
    ),
  );
});

it('never presents late discovery results for another query or a failed read as zero matches', async () => {
  let late!: (value: unknown) => void;
  mock.analysisDiscovery
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          late = resolve;
        }),
    )
    .mockRejectedValueOnce(new Error('查询失败'));
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await user.click(await screen.findByRole('button', { name: '大文件' }));
  await openFilters(user);
  await user.selectOptions(screen.getByLabelText('用途筛选'), 'unclassified');
  await screen.findByText(/读取失败，尚不能判断/);
  await act(async () =>
    late({ items: [{ ...node, id: 'late', name: '迟到目录' }], total: 1, offset: 0 }),
  );
  expect(screen.queryByText('迟到目录')).not.toBeInTheDocument();
  expect(screen.queryByText(/没有符合这些筛选/)).not.toBeInTheDocument();
  mock.analysisDiscovery.mockResolvedValue({
    items: [{ ...node, id: 'kept', name: '已读目录' }],
    total: 1,
    offset: 0,
  });
  await user.click(screen.getByRole('button', { name: '重试读取目录' }));
  await screen.findByText('已读目录');
  mock.analysisDiscovery.mockRejectedValue(new Error('再试失败'));
  await user.click(screen.getByRole('button', { name: '重新读取当前结果' }));
  await screen.findByText(/本次更新失败，保留上次已读列表/);
  expect(screen.getByText('已读目录')).toBeInTheDocument();
});

it('sends precise thresholds and clears restrictive empty filters explicitly', async () => {
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await user.click(await screen.findByRole('button', { name: '大目录' }));
  await openFilters(user);
  await user.selectOptions(screen.getByLabelText('最小大小'), '1073741824');
  await user.click(screen.getByLabelText('包含内部目录'));
  await waitFor(() =>
    expect(mock.analysisDiscovery).toHaveBeenLastCalledWith(
      'session',
      expect.objectContaining({ min_bytes: '1073741824', include_nested: true }),
    ),
  );
  await user.click(await screen.findByRole('button', { name: '清除筛选限制' }));
  await waitFor(() =>
    expect(mock.analysisDiscovery).toHaveBeenLastCalledWith(
      'session',
      expect.objectContaining({ min_bytes: '0', search: '', category: 'all' }),
    ),
  );
  expect(props.onStart).not.toHaveBeenCalled();
});

it('keeps applied filters when collapsed and clears them without clearing name search', async () => {
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await user.click(await screen.findByRole('button', { name: '大目录' }));
  expect(screen.getByLabelText('用途筛选')).not.toBeVisible();
  await user.type(screen.getByRole('searchbox'), '文件');
  await openFilters(user);
  await user.selectOptions(screen.getByLabelText('用途筛选'), 'archives');
  await user.selectOptions(screen.getByLabelText('最小大小'), '1073741824');
  await user.click(screen.getByText('筛选 · 2 项已应用'));
  expect(screen.getByLabelText('用途筛选')).not.toBeVisible();
  expect(screen.getByText('筛选 · 2 项已应用')).toBeVisible();
  await user.click(screen.getByRole('button', { name: '清除筛选' }));
  expect(screen.getByRole('searchbox')).toHaveValue('文件');
  await waitFor(() =>
    expect(mock.analysisDiscovery).toHaveBeenLastCalledWith(
      'session',
      expect.objectContaining({ search: '文件', category: 'all', min_bytes: '0' }),
    ),
  );
  expect(screen.queryByRole('button', { name: '清除筛选' })).not.toBeInTheDocument();
});

it('keeps same-query rows and scroll after refresh fails, but never labels them as a new query', async () => {
  const child = { ...node, id: 'saved', name: '已读资料', parent: '0' };
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items: [child],
    total: 1,
    offset: 0,
    generated_at: 10,
  });
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  const row = await screen.findByRole('button', { name: '已读资料' });
  const list = screen.getByRole('generic', { name: '目录与文件排行' });
  list.scrollTop = 88;
  mock.analysisChildren.mockRejectedValue(new Error('读取超时'));
  await user.click(screen.getByRole('button', { name: '重新读取当前结果' }));
  await screen.findByText(/本次更新失败，保留上次已读列表/);
  expect(screen.getByRole('button', { name: '已读资料' })).toBe(row);
  expect(list.scrollTop).toBe(88);
  await user.type(screen.getByRole('searchbox'), '另一个名称');
  await screen.findByText('读取失败，尚不能判断目录内容。');
  expect(screen.queryByRole('button', { name: '已读资料' })).not.toBeInTheDocument();
});

it('folds a previous result when choosing a new location and restores it without starting work', async () => {
  mock.analysisLocations.mockResolvedValue([
    location,
    { token: 'other', title: '另一个磁盘', path: 'D:\\' },
  ]);
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items: [{ ...node, id: 'keep', name: '原目录' }],
    total: 1,
    offset: 0,
  });
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  const row = await screen.findByRole('button', { name: '原目录' });
  await user.click(screen.getByText(/位置与重新分析/));
  await user.selectOptions(screen.getByRole('combobox', { name: '准备分析的位置' }), 'other');
  expect(row).not.toBeVisible();
  expect(screen.getByRole('region', { name: '上次分析' })).toHaveTextContent(location.path);
  expect(props.onStart).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: '查看上次结果' }));
  expect(row).toBeVisible();
  expect(screen.getByRole('combobox', { name: '准备分析的位置', hidden: true })).toHaveValue(
    location.token,
  );
});

it('keeps one same-volume baseline across directories and retains dated values on failure', async () => {
  const other = { token: 'folder', title: '同卷目录', path: 'C:\\other' };
  mock.analysisLocations.mockResolvedValue([location, other]);
  const sample = {
    location_token: location.token,
    volume: 'C:\\',
    volume_key: 'identity-1',
    available_bytes: '1000',
    sampled_at: 1,
    error: null,
  };
  mock.analysisContext
    .mockResolvedValueOnce(sample)
    .mockResolvedValueOnce({
      ...sample,
      location_token: other.token,
      available_bytes: '999',
      sampled_at: 2,
    })
    .mockRejectedValueOnce(new Error('暂时不可用'));
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={null} />);
  await screen.findByText(/当前用户可用空间/);
  await user.selectOptions(screen.getByRole('combobox', { name: '准备分析的位置' }), 'folder');
  expect(await screen.findByText(/这是同一磁盘的观察变化/)).toHaveTextContent('−1 B');
  await user.click(screen.getByRole('button', { name: '刷新磁盘空间' }));
  await screen.findByText(/本次刷新失败/);
  expect(screen.getByText(/上次成功查询可用空间/)).toHaveTextContent('999 B');
  expect(screen.queryByText(/这是同一磁盘的观察变化/)).not.toBeInTheDocument();
});

it('embeds ranking bars in the list once and preserves partial purposes', async () => {
  const items = Array.from({ length: 8 }, (_, i) => ({
    ...node,
    id: String(i + 1),
    parent: '0',
    name: `目录-${i}`,
    incomplete: i === 0,
    pending: i === 1,
    advice: { kind: 'system_data', app_id: null, basis: 'known_windows' },
  }));
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items,
    total: 8,
    offset: 0,
    coverage: { completed: 6, incomplete: 1, pending: 1 },
  });
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={{ ...snapshot, state: 'partial' }} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  await screen.findByText('系统、程序或共享数据 · 大小尚未统计完整');
  expect(document.querySelectorAll('.analysis-entry')).toHaveLength(8);
  expect(document.querySelectorAll('.entry-size-band')).toHaveLength(8);
  expect(screen.getByText(/本层已发现目录/)).toHaveTextContent('完成 6，未完成 1，待统计 1');
  expect(screen.queryByRole('button', { name: /进入目录切片/ })).not.toBeInTheDocument();
});

it('waits for explicit start and sends only a backend location token', async () => {
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={null} />);
  const button = await screen.findByRole('button', { name: '开始只读分析' });
  await waitFor(() => expect(button).toBeEnabled());
  expect(props.onStart).not.toHaveBeenCalled();
  await user.click(button);
  expect(props.onStart).toHaveBeenCalledWith('root-token');
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
});
it('keeps failed reads distinct from empty directories and supports retry', async () => {
  mock.analysisChildren.mockRejectedValueOnce(new Error('timeout'));
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  await screen.findByText('读取失败，尚不能判断目录内容。');
  expect(screen.queryByText('此目录没有文件或子目录。')).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '重试读取目录' }));
  expect(await screen.findByText('此目录没有文件或子目录。')).toBeVisible();
});
it('drills into directories and reveals only session-bound identifiers', async () => {
  const child = {
    ...node,
    id: '1',
    parent: '0',
    name: '工作资料',
    directory: true,
    excluded: true,
    shared: true,
  };
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items: [child],
    total: 1,
    offset: 0,
  });
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={{ ...snapshot, stale: true }} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  await user.click(await screen.findByRole('button', { name: /查看详情：/ }));
  await user.click(screen.getByText('更多操作'));
  if (!screen.getByText('更多操作').closest('details')?.open)
    await user.click(screen.getByText('更多操作'));
  await user.click(screen.getByRole('button', { name: '在资源管理器中定位' }));
  expect(mock.revealAnalysis).toHaveBeenCalledWith('session', '1');
  await user.click(screen.getByRole('button', { name: '关闭文件详情' }));
  await user.click(screen.getByRole('button', { name: '工作资料' }));
  await waitFor(() =>
    expect(mock.analysisChildren).toHaveBeenLastCalledWith('session', '1', '', 0, true),
  );
  expect(screen.getByText(/这是清理前的统计/)).toBeVisible();
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
});
it('does not call incomplete results an empty directory', async () => {
  render(<AnalysisPanel {...props} snapshot={{ ...snapshot, state: 'cancelled' }} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  expect(await screen.findByText('此目录尚未发现可显示的条目，统计并不完整。')).toBeVisible();
});

it('explains structured picker errors and preserves the previous location', async () => {
  mock.pickAnalysis.mockRejectedValueOnce({
    code: 'unsupported_volume',
    message: '仅支持本地固定 NTFS 卷',
  });
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={null} />);
  await waitFor(() => expect(screen.getByRole('button', { name: '开始只读分析' })).toBeEnabled());
  await user.click(screen.getByRole('button', { name: '选择目录' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('仅支持本地固定 NTFS 卷');
  expect(screen.getByRole('alert')).not.toHaveTextContent('[object Object]');
  mock.pickAnalysis.mockResolvedValueOnce(null);
  await user.click(screen.getByRole('button', { name: '选择目录' }));
  await user.click(screen.getByRole('button', { name: '开始只读分析' }));
  expect(props.onStart).toHaveBeenCalledWith('root-token');
});

it('hands off new browser rules without starting an analysis or deletion', async () => {
  const child = { ...node, id: '1', parent: '0', name: 'Cache_Data', rule_ids: ['brave.http'] };
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items: [child],
    total: 1,
    offset: 0,
  });
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  await user.click(await screen.findByRole('button', { name: /查看详情：/ }));
  await user.click(screen.getByRole('button', { name: '查看对应清理类别' }));
  expect(props.onCleanup).toHaveBeenCalledWith(['brave.http']);
  expect(props.onStart).not.toHaveBeenCalled();
});

it('keeps view searches separate, finds deep files, and returns to their parent', async () => {
  const user = userEvent.setup();
  const file = {
    ...node,
    id: 'deep-file',
    parent: 'deep-folder',
    directory: false,
    name: '深层大文件.txt',
    bytes: '9007199254740993',
  };
  mock.analysisDiscovery.mockResolvedValue({ items: [file], total: 1, offset: 0 });
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  await user.type(await screen.findByRole('searchbox', { name: '搜索当前目录名称' }), '目录');
  await waitFor(() =>
    expect(mock.analysisChildren).toHaveBeenLastCalledWith('session', '0', '目录', 0, true),
  );
  await user.click(screen.getByRole('button', { name: '大文件' }));
  await screen.findByText('深层大文件.txt');
  await user.type(screen.getByRole('searchbox', { name: '搜索本次范围的名称' }), '大文件');
  await waitFor(() =>
    expect(mock.analysisDiscovery).toHaveBeenLastCalledWith(
      'session',
      expect.objectContaining({ kind: 'file', search: '大文件', offset: 0 }),
    ),
  );
  await user.click(screen.getByRole('button', { name: '目录浏览' }));
  expect(screen.getByRole('searchbox')).toHaveValue('目录');
  await user.click(screen.getByRole('button', { name: '大文件' }));
  expect(screen.getByRole('searchbox')).toHaveValue('大文件');
  await user.click(await screen.findByRole('button', { name: /查看详情：/ }));
  await user.click(screen.getByText('更多操作'));
  await user.click(screen.getByRole('button', { name: '打开所在目录' }));
  await waitFor(() =>
    expect(mock.analysisChildren).toHaveBeenLastCalledWith('session', 'deep-folder', '', 0, true),
  );
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
});

it('discards a late file query after switching views and resets a new session', async () => {
  let resolveFiles!: (data: unknown) => void;
  mock.analysisDiscovery.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        resolveFiles = resolve;
      }),
  );
  const user = userEvent.setup();
  const rendered = render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await user.click(await screen.findByRole('button', { name: '大文件' }));
  await waitFor(() => expect(mock.analysisDiscovery).toHaveBeenCalled());
  await user.click(screen.getByRole('button', { name: '目录浏览' }));
  await act(async () => {
    resolveFiles({ items: [{ ...node, id: 'late', name: '迟到结果' }], total: 1, offset: 0 });
  });
  await screen.findByText('此目录没有文件或子目录。');
  expect(screen.queryByText('迟到结果')).not.toBeInTheDocument();
  await user.type(screen.getByRole('searchbox'), '旧搜索');
  rendered.rerender(<AnalysisPanel {...props} snapshot={{ ...snapshot, id: 'new-session' }} />);
  await waitFor(() => expect(screen.getByRole('searchbox')).toHaveValue(''));
});

it('shows unknown for failed disk queries and samples only on meaningful events', async () => {
  mock.analysisContext.mockResolvedValueOnce({
    location_token: location.token,
    volume: null,
    available_bytes: null,
    sampled_at: 1,
    error: '采样失败',
  });
  const user = userEvent.setup();
  const rendered = render(
    <AnalysisPanel {...props} snapshot={{ ...snapshot, state: 'scanning' }} />,
  );
  await screen.findByText('磁盘可用空间未知');
  await screen.findByText(/采样失败/);
  rendered.rerender(
    <AnalysisPanel {...props} snapshot={{ ...snapshot, state: 'scanning', checked: 300 }} />,
  );
  expect(mock.analysisContext).toHaveBeenCalledTimes(1);
  await user.click(screen.getByRole('button', { name: '重试磁盘查询' }));
  await screen.findByText(/当前用户可用空间/);
  expect(mock.analysisContext).toHaveBeenCalledTimes(2);
  rendered.rerender(<AnalysisPanel {...props} snapshot={snapshot} />);
  await waitFor(() => expect(mock.analysisContext).toHaveBeenCalledTimes(3));
});

it('discards disk information from a previously selected location', async () => {
  let resolveContext!: (data: unknown) => void;
  mock.analysisContext.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        resolveContext = resolve;
      }),
  );
  mock.analysisLocations.mockResolvedValue([
    location,
    { token: 'other', title: '文档', path: 'D:\\工作' },
  ]);
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={null} />);
  await waitFor(() => expect(mock.analysisContext).toHaveBeenCalledWith('root-token'));
  mock.analysisContext.mockResolvedValue({
    location_token: 'other',
    volume: 'D:\\',
    available_bytes: '100',
    sampled_at: 1,
    error: null,
  });
  await user.selectOptions(screen.getByRole('combobox', { name: '准备分析的位置' }), 'other');
  await screen.findByText(/D:.*当前用户可用空间/);
  await act(async () => {
    resolveContext({
      location_token: location.token,
      volume: 'OLD:',
      available_bytes: '999',
      sampled_at: 1,
      error: null,
    });
  });
  await waitFor(() => expect(screen.queryByText(/OLD:/)).not.toBeInTheDocument());
});

it('uses backend purposes and never guesses from WeChat, AppData or Cache names', async () => {
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items: [{ ...node, id: 'unknown', name: 'WeChat', path: 'D:\\AppData\\Cache\\WeChat' }],
    total: 1,
    offset: 0,
  });
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  expect(await screen.findByText('用途未确定')).toBeVisible();
  expect(screen.queryByText(/可能属于微信数据/)).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '查看对应清理类别' })).not.toBeInTheDocument();
});

it('offers fixed settings for system data and explains launch failure', async () => {
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items: [
      {
        ...node,
        id: 'system',
        advice: { kind: 'system_data', app_id: null, basis: 'known_windows' },
      },
    ],
    total: 1,
    offset: 0,
  });
  mock.apps.mockRejectedValueOnce({ code: 'failed', message: '设置不可用' });
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  await user.click(await screen.findByText('系统、程序或共享数据'));
  await user.click(screen.getByRole('button', { name: /查看详情：/ }));
  await user.click(screen.getByRole('button', { name: 'Windows 应用管理' }));
  expect(mock.apps).toHaveBeenCalledWith();
  expect(await screen.findByRole('alert')).toHaveTextContent('设置不可用');
});

it('defaults to the backend system disk, explains unsupported disks and never auto starts', async () => {
  mock.analysisLocations.mockResolvedValue([
    location,
    {
      token: 'data',
      title: '数据盘',
      path: 'G:\\',
      kind: 'volume',
      is_system: false,
      supported: true,
    },
    {
      token: 'system',
      title: '系统盘',
      path: 'C:\\',
      kind: 'volume',
      is_system: true,
      supported: true,
    },
    {
      token: 'fat',
      title: '旧磁盘',
      path: 'F:\\',
      kind: 'volume',
      supported: false,
      unavailable_reason: '首版仅支持 NTFS',
    },
  ]);
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={null} />);
  await waitFor(() =>
    expect(screen.getByRole('combobox', { name: '准备分析的位置' })).toHaveValue('system'),
  );
  expect(props.onStart).not.toHaveBeenCalled();
  await user.selectOptions(screen.getByRole('combobox', { name: '准备分析的位置' }), 'fat');
  expect(screen.getByText('首版仅支持 NTFS')).toBeVisible();
  expect(screen.getByRole('button', { name: '开始只读分析' })).toBeDisabled();
  await user.selectOptions(screen.getByRole('combobox', { name: '准备分析的位置' }), 'data');
  await user.click(screen.getByRole('button', { name: '开始只读分析' }));
  expect(props.onStart).toHaveBeenCalledWith('data');
});

it('keeps loaded rows, scroll and query when returning from cleanup', async () => {
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items: [{ ...node, id: 'cache', name: 'cache', rule_ids: ['chrome.cache'] }],
    total: 1,
    offset: 0,
  });
  const rendered = render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  const trigger = await screen.findByRole('button', { name: /查看详情：/ });
  const list = screen.getByRole('generic', { name: '目录与文件排行' });
  list.scrollTop = 150;
  const reads = mock.analysisChildren.mock.calls.length;
  rendered.rerender(<AnalysisPanel {...props} active={false} snapshot={snapshot} />);
  rendered.rerender(<AnalysisPanel {...props} active snapshot={snapshot} />);
  expect(screen.getByRole('button', { name: /查看详情：/ })).toBe(trigger);
  expect(list.scrollTop).toBe(150);
  expect(mock.analysisChildren).toHaveBeenCalledTimes(reads);
});

it('compares exact same-volume samples, shows negative growth and rejects a changed identity', async () => {
  const sample = {
    location_token: location.token,
    volume: 'C:\\',
    volume_key: 'volume-1',
    available_bytes: '9007199254740993',
    sampled_at: 1,
    error: null,
  };
  mock.analysisContext
    .mockResolvedValueOnce(sample)
    .mockResolvedValueOnce({ ...sample, available_bytes: '9007199254740992', sampled_at: 2 })
    .mockResolvedValueOnce({ ...sample, volume_key: 'volume-2', sampled_at: 3 });
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={null} />);
  await screen.findByText(/当前用户可用空间/);
  await user.click(screen.getByRole('button', { name: '刷新磁盘空间' }));
  expect(await screen.findByText(/这是同一磁盘的观察变化/)).toHaveTextContent('−1 B');
  await user.click(screen.getByRole('button', { name: '刷新磁盘空间' }));
  await waitFor(() => expect(screen.queryByText(/这是同一磁盘的观察变化/)).not.toBeInTheDocument());
  expect(props.onStart).not.toHaveBeenCalled();
});

function reviewRows() {
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items: [
      {
        ...node,
        id: 'first',
        name: 'Chrome资料',
        rule_ids: [],
        advice: { kind: 'browser_data', app_id: 'chrome', basis: 'standard_browser_data' },
      },
      { ...node, id: 'second', name: '另一项目', directory: false, rule_ids: ['weixin.logs'] },
    ],
    total: 2,
    offset: 0,
  });
}
it('keeps one review, restores search and focus, and ends without claiming deletion', async () => {
  reviewRows();
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  await user.type(await screen.findByRole('searchbox'), '资料');
  await waitFor(() =>
    expect(mock.analysisChildren).toHaveBeenLastCalledWith('session', '0', '资料', 0, true),
  );
  const trigger = (await screen.findAllByRole('button', { name: /查看详情：/ }))[0];
  trigger.focus();
  await user.click(trigger);
  let dialog = screen.getByRole('dialog');
  expect(within(dialog).queryByText(/存储管理指引/)).not.toBeInTheDocument();
  expect(within(dialog).getByRole('heading', { name: /Google Chrome/ })).toBeVisible();
  expect(within(dialog).queryByText(/Brave|Firefox|微信/)).not.toBeInTheDocument();
  expect(props.onCleanup).not.toHaveBeenCalled();
  await user.keyboard('{Escape}');
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(screen.getByRole('searchbox')).toHaveValue('资料');
  await user.click(screen.getAllByRole('button', { name: /查看详情：/ })[0]);
  dialog = screen.getByRole('dialog');
  await user.click(within(dialog).getByRole('button', { name: '返回列表' }));
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(screen.queryByRole('button', { name: '继续核对' })).not.toBeInTheDocument();
  expect(screen.queryByText(/本项核对已结束/)).not.toBeInTheDocument();
  expect(props.onStart).not.toHaveBeenCalled();
  expect(props.onCleanup).not.toHaveBeenCalled();
});
it('details do not sample space or start another analysis and close on session replacement', async () => {
  reviewRows();
  const user = userEvent.setup();
  const rendered = render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await user.click(await screen.findByRole('button', { name: '目录浏览' }));
  await waitFor(() => expect(mock.analysisContext).toHaveBeenCalled());
  const samples = mock.analysisContext.mock.calls.length;
  await user.click((await screen.findAllByRole('button', { name: /查看详情：/ }))[0]);
  const dialog = within(screen.getByRole('dialog'));
  expect(dialog.queryByRole('button', { name: '刷新磁盘空间' })).not.toBeInTheDocument();
  expect(dialog.queryByText('需要更新文件排行？')).not.toBeInTheDocument();
  expect(mock.analysisContext).toHaveBeenCalledTimes(samples);
  expect(props.onStart).not.toHaveBeenCalled();
  rendered.rerender(<AnalysisPanel {...props} snapshot={{ ...snapshot, id: 'replacement' }} />);
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(screen.queryByRole('button', { name: '继续核对' })).not.toBeInTheDocument();
});
it('replaces the target and ignores a late invalid-session failure from the old dialog', async () => {
  reviewRows();
  let reject!: (value: unknown) => void;
  mock.revealAnalysis.mockImplementationOnce(
    () =>
      new Promise((_, rejectPromise) => {
        reject = rejectPromise;
      }),
  );
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  await user.click((await screen.findAllByRole('button', { name: /查看详情：/ }))[0]);
  if (!screen.getByText('更多操作').closest('details')?.open)
    await user.click(screen.getByText('更多操作'));
  await user.click(screen.getByRole('button', { name: '在资源管理器中定位' }));
  await user.click(screen.getByRole('button', { name: '关闭文件详情' }));
  await user.click(screen.getAllByRole('button', { name: /查看详情：/ })[1]);
  await act(async () => reject({ code: 'invalid_session', message: '旧请求' }));
  expect(
    within(screen.getByRole('dialog')).getByRole('heading', { name: '另一项目' }),
  ).toBeVisible();
  expect(screen.queryByText('分析会话已失效，请重新分析。')).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '查看对应清理类别' }));
  expect(props.onCleanup).toHaveBeenCalledWith(['weixin.logs']);
  expect(screen.queryByRole('button', { name: '继续核对' })).not.toBeInTheDocument();
});
it('clears a current invalid session and never retries the old entry through a new one', async () => {
  reviewRows();
  mock.revealAnalysis.mockRejectedValueOnce({ code: 'invalid_session', message: '会话已失效' });
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  await user.click((await screen.findAllByRole('button', { name: /查看详情：/ }))[0]);
  if (!screen.getByText('更多操作').closest('details')?.open)
    await user.click(screen.getByText('更多操作'));
  await user.click(screen.getByRole('button', { name: '在资源管理器中定位' }));
  expect(await screen.findByText('分析会话已失效，请重新分析。')).toBeVisible();
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '继续核对' })).not.toBeInTheDocument();
  expect(mock.revealAnalysis).toHaveBeenLastCalledWith('session', 'first');
});

it('selects an incomplete directory without starting work and moves focus to the next action', async () => {
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items: [{ ...node, id: 'partial', name: '部分目录', incomplete: true }],
    total: 1,
    offset: 0,
  });
  mock.refineAnalysis.mockResolvedValue({
    token: 'refined',
    title: '部分目录',
    path: 'C:\\fixture\\partial',
  });
  const user = userEvent.setup();
  render(<AnalysisPanel {...props} snapshot={snapshot} />);
  await userEvent.setup().click(await screen.findByRole('button', { name: '目录浏览' }));
  await user.click(await screen.findByRole('button', { name: /查看详情：/ }));
  await user.click(screen.getByRole('button', { name: '选择此目录补充分析' }));
  const start = await screen.findByRole('button', { name: '开始只读分析' });
  await waitFor(() => expect(start).toHaveFocus());
  expect(mock.refineAnalysis).toHaveBeenCalledWith('session', 'partial');
  expect(props.onStart).not.toHaveBeenCalled();
  expect(screen.getByRole('combobox', { name: '准备分析的位置' })).toHaveValue('refined');
  expect(screen.queryByRole('button', { name: '继续核对' })).not.toBeInTheDocument();
});

it('shares checkboxes between file details and the list, batches pages once and keeps protected rows unselected', async () => {
  const file = {
    ...node,
    id: 'file',
    parent: '0',
    ancestors: ['0'],
    directory: false,
    name: '可选文件',
    manual: { allowed: true, reason: null },
  };
  const protectedFile = {
    ...file,
    id: 'protected',
    name: '保护文件',
    manual: { allowed: false, reason: 'protected' as const },
  };
  mock.analysisChildren.mockResolvedValue({
    node,
    breadcrumbs: [node],
    items: [file, protectedFile],
    total: 2,
    offset: 0,
  });
  const batch = vi.fn();
  function Harness() {
    const [choices, setChoices] = useState<ManualChoices>(new Map());
    return (
      <AnalysisPanel
        {...props}
        snapshot={snapshot}
        manualChoices={choices}
        onSelectManual={(entry, selected) => setChoices((c) => toggleManual(c, entry, selected))}
        onSelectManualBatch={(entries, selected) => {
          batch(entries, selected);
          setChoices((c) => toggleManualBatch(c, entries, selected));
        }}
      />
    );
  }
  const user = userEvent.setup();
  render(<Harness />);
  await user.click(await screen.findByRole('button', { name: '目录浏览' }));
  await user.click((await screen.findAllByRole('button', { name: /查看详情：/ }))[0]);
  const dialog = within(screen.getByRole('dialog'));
  expect(dialog.getByRole('checkbox', { name: '选择可选文件' })).not.toBeChecked();
  await user.click(dialog.getByRole('checkbox', { name: '选择可选文件' }));
  await user.keyboard('{Escape}');
  expect(screen.getByRole('checkbox', { name: '选择可选文件' })).toBeChecked();
  await user.click(screen.getByRole('button', { name: '取消本页选择' }));
  expect(batch).toHaveBeenCalledTimes(1);
  expect(screen.getByRole('checkbox', { name: '选择可选文件' })).not.toBeChecked();
  await user.click(screen.getByRole('button', { name: '选择本页可选项' }));
  expect(batch).toHaveBeenCalledTimes(2);
  expect(screen.getByRole('checkbox', { name: '选择可选文件' })).toBeChecked();
  expect(screen.getByRole('checkbox', { name: '选择保护文件' })).not.toBeChecked();
});
