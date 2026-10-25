import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { AttachmentPanel } from '../AttachmentPanel';
import { api } from '../api';
import type { AttachmentEntry, AttachmentSnapshot } from '../attachmentTypes';
vi.mock('../api', () => ({
  desktop: true,
  api: {
    attachmentLocations: vi.fn(),
    startAttachments: vi.fn(),
    attachmentSnapshot: vi.fn(),
    attachments: vi.fn(),
    attachmentThumbnail: vi.fn(),
    prepareAttachments: vi.fn(),
    closeAttachments: vi.fn(),
    cancelAttachmentPlan: vi.fn(),
    execute: vi.fn(),
    report: vi.fn(),
    revealAttachment: vi.fn(),
    cancelPreparation: vi.fn(),
    pickAttachment: vi.fn(),
  },
}));
const location = {
  provider: 'weixin' as const,
  token: 'location',
  path: 'C:\\test\\xwechat_files\\wxid_sample',
  title: '测试账号',
  version: '4.test',
  cleanup_block: null,
};
const snapshot: AttachmentSnapshot = {
  session: 'session',
  location,
  state: 'complete',
  files: 2,
  bytes: '300',
  issues: 0,
  error: null,
  deleted_ids: [],
};
const entries: AttachmentEntry[] = [
  {
    id: 'a',
    name: 'sample.png',
    path: 'C:\\test\\a.png',
    kind: 'image',
    bytes: '200',
    modified_unix: 1,
    preview_available: true,
    cleanup_block: null,
    outcome: null,
  },
  {
    id: 'b',
    name: 'private.dat',
    path: 'C:\\test\\b.dat',
    kind: 'unknown',
    bytes: '100',
    modified_unix: 1,
    preview_available: false,
    cleanup_block: '未知格式，已保留',
    outcome: null,
  },
];
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.attachmentLocations).mockResolvedValue({
    provider: 'weixin',
    locations: [location],
    state: 'ready',
    issues: [],
  });
  vi.mocked(api.startAttachments).mockResolvedValue('session');
  vi.mocked(api.attachmentSnapshot).mockResolvedValue(snapshot);
  vi.mocked(api.attachments).mockImplementation(async (q) => ({
    snapshot,
    total: q.kind === 'audio' ? 0 : 2,
    entries: q.kind === 'audio' ? [] : entries,
  }));
  vi.mocked(api.attachmentThumbnail).mockResolvedValue('data:image/png;base64,aGVsbG8=');
  vi.mocked(api.closeAttachments).mockResolvedValue();
  vi.mocked(api.prepareAttachments).mockResolvedValue([
    {
      scope: 'user',
      plan: {
        id: 'plan',
        session_id: 'session',
        files: 1,
        bytes: '200',
        rule_ids: [],
        impacts: [],
        categories: [],
      },
    },
  ]);
});
afterEach(cleanup);

it('keeps cross-page choices explicit with ten thousand records and ignores late filtered responses', async () => {
  const large = { ...snapshot, files: 10000, bytes: '1000000' };
  vi.mocked(api.attachmentSnapshot).mockResolvedValue(large);
  let late: ((value: Awaited<ReturnType<typeof api.attachments>>) => void) | undefined;
  const page = (offset: number, search = '') => ({
    snapshot: large,
    total: search ? 1 : 10000,
    entries: Array.from({ length: search ? 1 : 100 }, (_, i) => ({
      ...entries[0],
      id: `item-${offset + i}`,
      name: search || `item-${offset + i}.png`,
      bytes: '100',
    })),
  });
  vi.mocked(api.attachments).mockImplementation(async (q) => {
    if (q.search === 'late')
      return new Promise((resolve) => {
        late = resolve;
      });
    return page(q.offset, q.search);
  });
  render(<AttachmentPanel onClose={vi.fn()} />);
  await screen.findByRole('option', { name: /测试账号/ });
  fireEvent.change(screen.getByLabelText('账号资料位置'), { target: { value: 'location' } });
  await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled());
  fireEvent.click(await screen.findByRole('button', { name: '开始扫描' }));
  fireEvent.click(await screen.findByRole('checkbox', { name: '选择item-0.png' }));
  fireEvent.click(screen.getByRole('button', { name: '下一页' }));
  fireEvent.click(await screen.findByRole('checkbox', { name: '选择item-100.png' }));
  expect(screen.getByText(/已选 2 个文件/)).toBeVisible();
  fireEvent.change(screen.getByLabelText('文件名'), { target: { value: 'late' } });
  await waitFor(() => expect(late).toBeDefined());
  fireEvent.change(screen.getByLabelText('文件名'), { target: { value: 'current' } });
  await screen.findByRole('checkbox', { name: '选择current' });
  await act(async () => {
    late!(page(500, 'stale'));
  });
  expect(screen.queryByRole('checkbox', { name: '选择stale' })).not.toBeInTheDocument();
  expect(screen.getByText(/已选 2 个文件/)).toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: '清理所选附件' }));
  await waitFor(() =>
    expect(api.prepareAttachments).toHaveBeenCalledWith(
      { session: 'session', selected_ids: ['item-0', 'item-100'] },
      expect.any(String),
    ),
  );
});
it('shows classification totals outside the age filter without selecting retained attachments', async () => {
  vi.mocked(api.attachments).mockImplementation(async (q) => ({
    snapshot,
    total: q.min_age_days ? 0 : 2,
    entries: q.min_age_days ? [] : entries,
    filtered_bytes: q.min_age_days ? '0' : '300',
    selectable_files: q.min_age_days ? 0 : 1,
    selectable_bytes: q.min_age_days ? '0' : '200',
    categories: [
      { kind: 'image', files: 1, bytes: '200', retained_files: 0, retained_bytes: '0' },
      { kind: 'unknown', files: 1, bytes: '100', retained_files: 1, retained_bytes: '100' },
    ],
  }));
  render(<AttachmentPanel onClose={vi.fn()} />);
  await screen.findByRole('option', { name: /测试账号/ });
  fireEvent.change(screen.getByLabelText('账号资料位置'), { target: { value: 'location' } });
  await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled());
  fireEvent.click(screen.getByRole('button', { name: '开始扫描' }));
  await screen.findByLabelText('附件分类占用');
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '查看全部时间' }));
  expect(await screen.findByRole('checkbox', { name: '选择sample.png' })).not.toBeChecked();
  expect(screen.getByRole('checkbox', { name: '选择private.dat' })).toBeDisabled();
  expect(api.prepareAttachments).not.toHaveBeenCalled();
});
async function start() {
  render(<AttachmentPanel onClose={vi.fn()} />);
  await screen.findByRole('option', { name: /测试账号/ });
  fireEvent.change(screen.getByLabelText('账号资料位置'), { target: { value: 'location' } });
  await waitFor(() => expect(screen.getByRole('button', { name: '开始扫描' })).toBeEnabled());
  fireEvent.click(screen.getByRole('button', { name: '开始扫描' }));
  await screen.findByRole('checkbox', { name: '选择sample.png' });
}
it('starts only on request, defaults to 30 days, and never preselects attachments', async () => {
  await start();
  expect(api.startAttachments).toHaveBeenCalledTimes(1);
  expect(api.attachments).toHaveBeenLastCalledWith(
    expect.objectContaining({ min_age_days: 30, selected_ids: null }),
  );
  expect(screen.getByRole('checkbox', { name: '选择sample.png' })).not.toBeChecked();
  expect(screen.getByRole('checkbox', { name: '选择private.dat' })).toBeDisabled();
  const preview = await screen.findByRole('button', { name: '预览sample.png' });
  await waitFor(() => expect(preview).toBeEnabled());
  fireEvent.click(preview);
  expect(screen.getByText('预览不会改变选择')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '关闭预览' }));
  expect(screen.getByRole('checkbox', { name: '选择sample.png' })).not.toBeChecked();
});
it('keeps hidden selections and freezes an independent attachment plan before confirmation', async () => {
  await start();
  fireEvent.click(screen.getByRole('checkbox', { name: '选择sample.png' }));
  fireEvent.change(screen.getByLabelText('类型'), { target: { value: 'audio' } });
  await screen.findByText('当前筛选下没有附件。可以调整时间或类型。');
  expect(screen.getByText(/已选 1 个文件/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '清理所选附件' }));
  await screen.findByRole('button', { name: '确认永久删除' });
  expect(api.prepareAttachments).toHaveBeenCalledWith(
    { session: 'session', selected_ids: ['a'] },
    expect.any(String),
  );
  expect(api.execute).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '返回选择' }));
  expect(screen.getByText(/已选 1 个文件/)).toBeInTheDocument();
});
it('shows thumbnail errors without disabling other browsing', async () => {
  vi.mocked(api.attachmentThumbnail).mockRejectedValue(new Error('unsupported codec'));
  await start();
  await screen.findByText(/预览不可用.*unsupported codec/);
  expect(screen.getByRole('checkbox', { name: '选择sample.png' })).toBeEnabled();
});

it('clears the old selection and session when switching application without scanning', async () => {
  await start();
  fireEvent.click(screen.getByRole('checkbox', { name: '选择sample.png' }));
  vi.mocked(api.attachmentLocations).mockResolvedValue({
    provider: 'qq_classic',
    locations: [],
    state: 'not_installed',
    issues: [],
  });
  fireEvent.change(screen.getByLabelText('应用'), { target: { value: 'qq_classic' } });
  await screen.findByText('未检测到经典 QQ安装。');
  expect(screen.getByText(/已选 0 个文件/)).toBeInTheDocument();
  expect(screen.queryByRole('checkbox', { name: '选择sample.png' })).not.toBeInTheDocument();
  expect(api.closeAttachments).toHaveBeenCalledTimes(2);
  expect(api.startAttachments).toHaveBeenCalledTimes(1);
  expect(api.attachmentLocations).toHaveBeenLastCalledWith('qq_classic');
});

it('ignores a late discovery response from a different application', async () => {
  let finish!: (value: Awaited<ReturnType<typeof api.attachmentLocations>>) => void;
  vi.mocked(api.attachmentLocations).mockImplementation((provider) =>
    provider === 'weixin'
      ? new Promise((resolve) => {
          finish = resolve;
        })
      : Promise.resolve({ provider, locations: [], state: 'no_accounts', issues: [] }),
  );
  render(<AttachmentPanel onClose={vi.fn()} />);
  fireEvent.change(screen.getByLabelText('应用'), { target: { value: 'qqnt' } });
  await screen.findByText(/未发现QQNT账号资料/);
  await act(async () =>
    finish({ provider: 'weixin', locations: [location], state: 'ready', issues: [] }),
  );
  expect(screen.queryByRole('option', { name: /测试账号/ })).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '开始扫描' })).toBeDisabled();
});

it('retires the frozen confirmation when changing application', async () => {
  await start();
  fireEvent.click(screen.getByRole('checkbox', { name: '选择sample.png' }));
  fireEvent.click(screen.getByRole('button', { name: '清理所选附件' }));
  await screen.findByRole('button', { name: '确认永久删除' });
  vi.mocked(api.attachmentLocations).mockResolvedValue({
    provider: 'qqnt',
    locations: [],
    state: 'no_accounts',
    issues: [],
  });
  fireEvent.change(screen.getByLabelText('应用'), { target: { value: 'qqnt' } });
  await screen.findByText(/未发现QQNT账号资料/);
  expect(screen.queryByRole('button', { name: '确认永久删除' })).not.toBeInTheDocument();
  expect(screen.getByText(/已选 0 个文件/)).toBeInTheDocument();
  expect(api.execute).not.toHaveBeenCalled();
  expect(api.closeAttachments).toHaveBeenCalledTimes(2);
});

it('does not render a late page from the previous account', async () => {
  vi.mocked(api.attachmentLocations).mockResolvedValue({
    provider: 'weixin',
    locations: [location, { ...location, token: 'second', title: '第二账号' }],
    state: 'ready',
    issues: [],
  });
  await start();
  let finish!: (value: Awaited<ReturnType<typeof api.attachments>>) => void;
  vi.mocked(api.attachments).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  fireEvent.change(screen.getByLabelText('文件名'), { target: { value: 'late' } });
  await waitFor(() => expect(finish).toBeDefined());
  fireEvent.change(screen.getByLabelText('账号资料位置'), { target: { value: 'second' } });
  await waitFor(() => expect(screen.getByLabelText('账号资料位置')).toHaveValue('second'));
  await act(async () => finish({ snapshot, entries, total: 2 }));
  expect(screen.queryByRole('checkbox', { name: '选择sample.png' })).not.toBeInTheDocument();
  expect(screen.getByText(/已选 0 个文件/)).toBeInTheDocument();
});

it.each(['qq_classic', 'qqnt'] as const)(
  'passes the chosen family to custom location validation: %s',
  async (provider) => {
    vi.mocked(api.attachmentLocations).mockImplementation(async (p) => ({
      provider: p,
      locations: [],
      state: 'no_accounts',
      issues: [],
    }));
    vi.mocked(api.pickAttachment).mockResolvedValue(null);
    render(<AttachmentPanel onClose={vi.fn()} />);
    fireEvent.change(screen.getByLabelText('应用'), { target: { value: provider } });
    await waitFor(() => expect(screen.getByRole('button', { name: '选择其他位置' })).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: '选择其他位置' }));
    await waitFor(() => expect(api.pickAttachment).toHaveBeenCalledWith(provider));
    expect(api.startAttachments).not.toHaveBeenCalled();
  },
);

it('refreshes outcomes after the final report and clears only successful selections', async () => {
  const selectable = entries.map((item) => ({ ...item, cleanup_block: null }));
  vi.mocked(api.attachments).mockResolvedValue({ snapshot, total: 2, entries: selectable });
  await start();
  fireEvent.click(screen.getByRole('checkbox', { name: '选择sample.png' }));
  fireEvent.click(screen.getByRole('checkbox', { name: '选择private.dat' }));
  fireEvent.click(screen.getByRole('button', { name: '清理所选附件' }));
  await screen.findByRole('button', { name: '确认永久删除' });
  vi.mocked(api.execute).mockResolvedValue({ scopes: ['user'], warning: null });
  vi.mocked(api.report).mockImplementation(async () => {
    // Completion occurs after the first snapshot was read.
    vi.mocked(api.attachmentSnapshot).mockResolvedValue({ ...snapshot, deleted_ids: ['a'] });
    return {
      id: 'plan',
      scope: 'user',
      started_at: 1,
      finished_at: 2,
      status: 'finished',
      total: 2,
      deleted: 1,
      skipped: 1,
      failed: 0,
      unknown: 0,
      candidate_bytes: '300',
      deleted_bytes: '200',
      free_space_change: null,
      categories: [],
    };
  });
  fireEvent.click(screen.getByRole('button', { name: '确认永久删除' }));
  await waitFor(() => expect(screen.getByText(/已选 1 个文件/)).toBeInTheDocument());
  expect(await screen.findByRole('checkbox', { name: '选择sample.png' })).not.toBeChecked();
  expect(await screen.findByRole('checkbox', { name: '选择private.dat' })).toBeChecked();
  expect(screen.getByText(/本次清理结果，以上分类仍为扫描时记录/)).toBeVisible();
});

it('labels historical category capacity and shows deletion separately from retained files', async () => {
  vi.mocked(api.attachments).mockResolvedValue({
    snapshot,
    total: 2,
    entries: [{ ...entries[0], outcome: 'deleted', cleanup_block: '本次已处理' }, entries[1]],
    categories: [
      {
        kind: 'image',
        files: 1,
        bytes: '200',
        retained_files: 0,
        retained_bytes: '0',
        deleted_files: 1,
        deleted_bytes: '200',
      },
      {
        kind: 'unknown',
        files: 1,
        bytes: '100',
        retained_files: 1,
        retained_bytes: '100',
        deleted_files: 0,
        deleted_bytes: '0',
      },
    ],
  });
  await start();
  expect(screen.getByText(/分类为扫描时占用/)).toBeVisible();
  expect(screen.getByText('本次已删除 1 个 · 200 B')).toBeVisible();
  expect(screen.getByText('已保留或待重新核验 1 个 · 100 B')).toBeVisible();
  expect(screen.getByRole('checkbox', { name: '选择sample.png' })).toBeDisabled();
});
