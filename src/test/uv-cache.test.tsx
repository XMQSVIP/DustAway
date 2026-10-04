import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen, renderHook, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { UvCacheAction, UvReceipt, UvTaskNotice, useUvTask } from '../UvCache';
import { api } from '../api';
import type { AppController } from '../useAppController';
vi.mock('../api', () => ({
  desktop: true,
  api: {
    prepareUv: vi.fn(),
    executeUv: vi.fn(),
    cancelUv: vi.fn(),
    cancelUvPreparation: vi.fn(),
    uvTask: vi.fn(),
  },
}));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
const plan = {
  id: 'one-use',
  session: 'scan',
  cache: 'C:\\Users\\Test\\uv\\cache',
  executable: 'C:\\Users\\Test\\.local\\bin\\uv.exe',
  expires_at: 9999999999,
};
function model() {
  return {
    sessions: [{ scope: 'user', id: 'scan' }],
    summaryRows: [{ rule_id: 'dev.uv', revision: 1, tool: { ready: true, message: 'ready' } }],
    refreshRule: vi.fn(),
    reconcile: vi.fn(),
    busy: false,
    saving: false,
    uv: { refresh: vi.fn().mockResolvedValue(undefined) },
    setHistoryEpoch: vi.fn(),
  } as unknown as AppController;
}
it('prepares once, explains shared size, cancels without executing and restores focus', async () => {
  vi.mocked(api.prepareUv).mockResolvedValue(plan);
  const user = userEvent.setup();
  render(<UvCacheAction model={model()} />);
  await user.click(screen.getByRole('button', { name: '整理 uv 闲置缓存' }));
  expect(await screen.findByRole('dialog')).toHaveTextContent(
    '扫描发现的总大小不等于本次能释放的大小',
  );
  expect(screen.getByRole('dialog')).toHaveTextContent('集中式项目环境');
  await user.click(screen.getByRole('button', { name: '取消' }));
  expect(api.executeUv).not.toHaveBeenCalled();
  // The initiating button remains the keyboard return target.
  expect(screen.getByRole('button', { name: '整理 uv 闲置缓存' })).toHaveFocus();
});
it('requires confirmation, submits a plan token only and does not replay on completion', async () => {
  vi.mocked(api.prepareUv).mockResolvedValue(plan);
  vi.mocked(api.executeUv).mockResolvedValue('one-use');
  const user = userEvent.setup();
  const state = model();
  render(<UvCacheAction model={state} />);
  await user.click(screen.getByRole('button', { name: '整理 uv 闲置缓存' }));
  await screen.findByRole('dialog');
  expect(api.executeUv).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: '确认整理闲置缓存' }));
  expect(api.executeUv).toHaveBeenCalledExactlyOnceWith('one-use');
  expect(state.uv.refresh).toHaveBeenCalledOnce();
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
});
it('shows a preparation failure and permits a fresh check without starting deletion', async () => {
  vi.mocked(api.prepareUv)
    .mockRejectedValueOnce(new Error('uv 正在运行'))
    .mockResolvedValueOnce(plan);
  const user = userEvent.setup();
  render(<UvCacheAction model={model()} />);
  await user.click(screen.getByRole('button', { name: '整理 uv 闲置缓存' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('uv 正在运行');
  expect(api.executeUv).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: '整理 uv 闲置缓存' }));
  expect(await screen.findByRole('dialog')).toBeInTheDocument();
});
it('discards a rejected confirmation and requires a fresh plan before retrying', async () => {
  vi.mocked(api.prepareUv)
    .mockResolvedValueOnce(plan)
    .mockResolvedValueOnce({ ...plan, id: 'fresh' });
  vi.mocked(api.executeUv).mockRejectedValueOnce(new Error('确认已失效，请重新检查'));
  const user = userEvent.setup();
  render(<UvCacheAction model={model()} />);
  await user.click(screen.getByRole('button', { name: '整理 uv 闲置缓存' }));
  await user.click(await screen.findByRole('button', { name: '确认整理闲置缓存' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('确认已失效');
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(api.executeUv).toHaveBeenCalledExactlyOnceWith('one-use');
  await user.click(screen.getByRole('button', { name: '整理 uv 闲置缓存' }));
  expect(await screen.findByRole('dialog')).toBeInTheDocument();
  expect(api.prepareUv).toHaveBeenCalledTimes(2);
  expect(api.executeUv).toHaveBeenCalledOnce();
});
it('does not label a logical size decrease as deleted or physically reclaimed bytes', () => {
  render(
    <UvReceipt
      report={{
        id: 'one-use',
        started_at: 1,
        finished_at: 2,
        status: 'completed',
        cache: plan.cache,
        command: 'uv cache prune',
        message: '整理完成',
        output: 'Removed unused entries',
        before_bytes: '8192',
        after_bytes: '2048',
        free_before: '100000',
        free_after: '90000',
      }}
    />,
  );
  expect(screen.getByText(/差值不代表实际释放磁盘空间/)).toBeInTheDocument();
  expect(screen.queryByText(/已删除 6/)).not.toBeInTheDocument();
});

it('invalidates only the existing scan and a recovered task never submits cleanup', async () => {
  const task = { id: 'recovered', session: 'user:first', status: 'completed' } as never;
  vi.mocked(api.uvTask).mockResolvedValue(task);
  const hook = renderHook(({ session }) => useUvTask(true, session), {
    initialProps: { session: 'user:first' },
  });
  await waitFor(() => expect(hook.result.current.stale).toBe(true));
  hook.rerender({ session: 'user:rescanned' });
  expect(hook.result.current.stale).toBe(false);
  await act(() => hook.result.current.refresh());
  expect(hook.result.current.stale).toBe(false);
  expect(api.executeUv).not.toHaveBeenCalled();
});

it('keeps a stale scan blocked after task lookup fails and recovers without replay', async () => {
  vi.mocked(api.uvTask).mockRejectedValueOnce(new Error('connection lost')).mockResolvedValue(null);
  const hook = renderHook(() => useUvTask(true, 'user:first'));
  act(() => hook.result.current.invalidate('user:first'));
  await waitFor(() => expect(hook.result.current.error).toContain('connection lost'));
  await act(() => hook.result.current.refresh());
  expect(hook.result.current.error).toBe('');
  expect(hook.result.current.stale).toBe(true);
  expect(api.executeUv).not.toHaveBeenCalled();
});

it('requests cancellation of the existing task and only refreshes its receipt', async () => {
  const state = model();
  state.uv.task = {
    session: 'scan',
    id: 'existing-task',
    started_at: 1,
    finished_at: null,
    status: 'running',
    cache: plan.cache,
    command: 'uv cache prune',
    message: '正在整理',
    output: '',
    before_bytes: '8192',
    after_bytes: null,
    free_before: null,
    free_after: null,
  };
  vi.mocked(api.cancelUv).mockResolvedValue(undefined);
  render(<UvTaskNotice model={state} />);
  await userEvent.setup().click(screen.getByRole('button', { name: '停止整理' }));
  expect(api.cancelUv).toHaveBeenCalledOnce();
  expect(state.uv.refresh).toHaveBeenCalledOnce();
  expect(api.prepareUv).not.toHaveBeenCalled();
  expect(api.executeUv).not.toHaveBeenCalled();
});

it('blocks a second confirmation while the first submission is unresolved', async () => {
  vi.mocked(api.prepareUv).mockResolvedValue(plan);
  let finish!: (id: string) => void;
  vi.mocked(api.executeUv).mockImplementation(
    () => new Promise<string>((resolve) => (finish = resolve)),
  );
  const user = userEvent.setup();
  render(<UvCacheAction model={model()} />);
  await user.click(screen.getByRole('button', { name: '整理 uv 闲置缓存' }));
  const confirm = await screen.findByRole('button', { name: '确认整理闲置缓存' });
  await user.dblClick(confirm);
  expect(api.executeUv).toHaveBeenCalledExactlyOnceWith('one-use');
  await act(async () => finish('one-use'));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(api.executeUv).toHaveBeenCalledOnce();
});
