import { cleanup, render, screen, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { FilesDialog, type FileViewState } from '../FilesDialog';
import rules from '../previewRules';
import type { CandidatePage } from '../types';
const mock = vi.hoisted(() => ({ candidates: vi.fn(), reveal: vi.fn() }));
vi.mock('../api', () => ({ api: mock }));
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 96,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({ index, start: index * 96 })),
    measureElement: () => {},
    measure: () => {},
  }),
}));
afterEach(cleanup);
beforeEach(() => {
  mock.candidates.mockReset();
  mock.reveal.mockReset();
});

it('keeps read files, selection and retryable locating after a reveal failure', async () => {
  const file = {
    id: 'file',
    rule_id: rules[0].id,
    path: 'C:/fixture/keep.tmp',
    bytes: '1024',
    modified_unix: 0,
    status: 'candidate',
    reason: null,
    reason_code: null,
  };
  mock.candidates.mockResolvedValue({ items: [file], total: 1, offset: 0 });
  mock.reveal.mockRejectedValueOnce(new Error('changed')).mockResolvedValueOnce(undefined);
  const onSelect = vi.fn();
  const user = userEvent.setup();
  render(
    <FilesDialog
      rule={rules[0]}
      scope="user"
      session="current"
      excluded={new Map()}
      editable
      actionsEnabled
      cleanup={false}
      onSelect={onSelect}
      onClose={vi.fn()}
    />,
  );
  await screen.findByText('keep.tmp');
  await user.click(screen.getByText('文件操作'));
  await user.click(screen.getByRole('button', { name: '在文件资源管理器中定位' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('文件明细与选择已保留');
  expect(screen.getByText('keep.tmp')).toBeVisible();
  expect(screen.getByRole('checkbox')).toBeChecked();
  expect(screen.queryByText('暂时无法读取文件明细，请重试。')).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '在文件资源管理器中定位' }));
  expect(mock.reveal).toHaveBeenCalledTimes(2);
  expect(mock.candidates).toHaveBeenCalledTimes(1);
  expect(onSelect).not.toHaveBeenCalled();
});
it('debounces literal searches and ignores an earlier response that arrives last', async () => {
  let resolveOld!: (p: CandidatePage) => void;
  mock.candidates.mockImplementation((_scope, _session, _rule, _offset, query) =>
    query.search === ''
      ? new Promise<CandidatePage>((resolve) => {
          resolveOld = resolve;
        })
      : Promise.resolve({ items: [], total: 7, offset: 0 }),
  );
  const user = userEvent.setup();
  render(
    <FilesDialog
      rule={rules[0]}
      scope="user"
      session="current"
      excluded={new Map()}
      editable
      cleanup={false}
      onSelect={vi.fn()}
      onClose={vi.fn()}
    />,
  );
  await user.type(screen.getByRole('searchbox'), '%_中文');
  await waitFor(() =>
    expect(mock.candidates).toHaveBeenLastCalledWith(
      'user',
      'current',
      rules[0].id,
      0,
      expect.objectContaining({ search: '%_中文' }),
    ),
  );
  expect(await screen.findByText(/共 7 项/)).toBeVisible();
  resolveOld({ items: [], total: 999, offset: 0 });
  await waitFor(() => expect(screen.getByText(/共 7 项/)).toBeVisible());
  expect(screen.queryByText(/共 999 项/)).not.toBeInTheDocument();
});

it('page selection only submits visible page ids and keeps filters across paging', async () => {
  const files = Array.from({ length: 2 }, (_, i) => ({
    id: `f${i}`,
    rule_id: rules[0].id,
    path: `C:/x/${i}`,
    bytes: '1',
    modified_unix: 0,
    status: 'candidate',
    reason: null,
    reason_code: null,
  }));
  mock.candidates.mockResolvedValue({ items: files, total: 202, offset: 0 });
  const onSelect = vi.fn();
  const user = userEvent.setup();
  render(
    <FilesDialog
      rule={rules[0]}
      scope="user"
      session="current"
      excluded={new Map()}
      editable
      cleanup={false}
      onSelect={onSelect}
      onClose={vi.fn()}
    />,
  );
  await waitFor(() => expect(screen.getByRole('button', { name: '取消本页选择' })).toBeEnabled());
  await user.click(screen.getByRole('button', { name: '取消本页选择' }));
  expect(onSelect).toHaveBeenCalledWith(files, false, 'user', 'current');
  await user.click(screen.getByRole('button', { name: '下一页' }));
  await waitFor(() =>
    expect(mock.candidates).toHaveBeenLastCalledWith(
      'user',
      'current',
      rules[0].id,
      200,
      expect.objectContaining({ sort: 'size' }),
    ),
  );
  expect(onSelect).toHaveBeenCalledTimes(1);
});

it('distinguishes a failed read from an empty category and allows retry', async () => {
  mock.candidates
    .mockRejectedValueOnce(new Error('read timeout'))
    .mockResolvedValue({ items: [], total: 0, offset: 0 });
  const user = userEvent.setup();
  render(
    <FilesDialog
      rule={rules[0]}
      scope="user"
      session="current"
      excluded={new Map()}
      editable={false}
      cleanup={false}
      onSelect={vi.fn()}
      onClose={vi.fn()}
    />,
  );
  expect(await screen.findByText('暂时无法读取文件明细，请重试。')).toBeVisible();
  expect(screen.queryByText('该类别没有可清理的文件。')).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '重试读取明细' }));
  expect(await screen.findByText('该类别没有可清理的文件。')).toBeVisible();
});

it('explains an expired session without querying a replacement or offering retry', async () => {
  mock.candidates.mockRejectedValue({ code: 'invalid_session', message: '扫描会话已失效' });
  render(
    <FilesDialog
      rule={rules[0]}
      scope="user"
      session="expired"
      excluded={new Map()}
      editable={false}
      cleanup
      initialStatus="failed"
      onSelect={vi.fn()}
      onClose={vi.fn()}
    />,
  );
  expect(await screen.findByText(/当前会话明细已不可用/)).toBeVisible();
  expect(screen.queryByRole('button', { name: '重试读取明细' })).not.toBeInTheDocument();
  expect(mock.candidates).toHaveBeenCalledTimes(1);
});
it('resets a new target and ignores a late response from the previous report', async () => {
  let finishOld!: (value: CandidatePage) => void;
  mock.candidates.mockImplementation((_scope, session) =>
    session === 'old'
      ? new Promise<CandidatePage>((resolve) => {
          finishOld = resolve;
        })
      : Promise.resolve({ items: [], total: 2, offset: 0 }),
  );
  const props = {
    rule: rules[0],
    scope: 'user' as const,
    excluded: new Map(),
    editable: false,
    cleanup: true,
    onSelect: vi.fn(),
    onClose: vi.fn(),
  };
  const user = userEvent.setup();
  const rendered = render(
    <FilesDialog key="old" {...props} session="old" initialStatus="failed" />,
  );
  await user.type(screen.getByRole('searchbox'), 'previous');
  rendered.rerender(<FilesDialog key="new" {...props} session="new" initialStatus="unknown" />);
  expect(screen.getByRole('searchbox')).toHaveValue('');
  await screen.findByText(/共 2 项/);
  finishOld({ items: [], total: 999, offset: 0 });
  await waitFor(() =>
    expect(mock.candidates).toHaveBeenLastCalledWith(
      'user',
      'new',
      rules[0].id,
      0,
      expect.objectContaining({ search: '', view: 'cleanup', status: 'unknown' }),
    ),
  );
  expect(screen.queryByText(/共 999 项/)).not.toBeInTheDocument();
});

it('restores search, sort, page and scroll per session without changing file selection', async () => {
  const file = {
    id: 'file',
    rule_id: rules[0].id,
    path: 'C:/fixture/keep.tmp',
    bytes: '1024',
    modified_unix: 0,
    status: 'candidate',
    reason: null,
    reason_code: null,
  };
  mock.candidates.mockResolvedValue({ items: [file], total: 401, offset: 0 });
  const viewCache = new Map<string, FileViewState>();
  const onSelect = vi.fn(),
    user = userEvent.setup();
  const props = {
    rule: rules[0],
    scope: 'user' as const,
    session: 'current',
    excluded: new Map(),
    editable: true,
    cleanup: false,
    onSelect,
    onClose: vi.fn(),
    viewCache,
  };
  const first = render(<FilesDialog {...props} />);
  await screen.findByText('keep.tmp');
  await user.type(screen.getByRole('searchbox'), 'keep');
  await waitFor(() =>
    expect(mock.candidates).toHaveBeenLastCalledWith(
      'user',
      'current',
      rules[0].id,
      0,
      expect.objectContaining({ search: 'keep' }),
    ),
  );
  await user.selectOptions(screen.getByLabelText('排序'), 'path');
  await waitFor(() => expect(screen.getByRole('button', { name: '下一页' })).toBeEnabled());
  await user.click(screen.getByRole('button', { name: '下一页' }));
  await waitFor(() => expect(screen.getByRole('button', { name: '下一页' })).toBeEnabled());
  const list = screen.getByLabelText('文件列表，可使用方向键滚动');
  fireEvent.scroll(list, { target: { scrollTop: 120 } });
  first.unmount();
  const second = render(<FilesDialog {...props} />);
  await screen.findByText('keep.tmp');
  expect(screen.getByRole('searchbox')).toHaveValue('keep');
  expect(screen.getByLabelText('排序')).toHaveValue('path');
  expect(mock.candidates).toHaveBeenLastCalledWith(
    'user',
    'current',
    rules[0].id,
    200,
    expect.objectContaining({ search: 'keep', sort: 'path' }),
  );
  expect(screen.getByLabelText('文件列表，可使用方向键滚动').scrollTop).toBe(120);
  second.unmount();
  render(<FilesDialog {...props} session="next" />);
  await screen.findByText('keep.tmp');
  expect(screen.getByRole('searchbox')).toHaveValue('');
  expect(mock.candidates).toHaveBeenLastCalledWith(
    'user',
    'next',
    rules[0].id,
    0,
    expect.objectContaining({ search: '', sort: 'size' }),
  );
  expect(onSelect).not.toHaveBeenCalled();
});
