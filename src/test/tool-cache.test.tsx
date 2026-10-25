import { afterEach, expect, it, vi } from 'vitest';
import { act, cleanup, render, renderHook, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ToolCacheCard, useToolTask, type ToolPlan, type ToolReport } from '../ToolCache';
import { CleanupEvidence } from '../CleanupEvidence';
import { api } from '../api';
import type { AppController } from '../useAppController';
vi.mock('../api', () => ({
  desktop: true,
  api: {
    prepareTool: vi.fn(),
    executeTool: vi.fn(),
    cancelTool: vi.fn(),
    cancelToolPreparation: vi.fn().mockResolvedValue(undefined),
    toolTask: vi.fn(),
  },
}));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
const plan: ToolPlan = {
  id: 'one-use',
  rule_id: 'dev.go_build',
  title: '清空 Go 构建缓存',
  session: 'scan',
  cache: 'C:\\Users\\Test\\go-build',
  executable: 'C:\\Go\\bin\\go.exe',
  expires_at: 9999999999,
};
function model() {
  return {
    sessions: [{ scope: 'user', id: 'scan' }],
    summaryRows: [
      { rule_id: 'dev.go_build', revision: 1, tool: { ready: true, message: 'ready' } },
    ],
    refreshRule: vi.fn(),
    reconcile: vi.fn(),
    decision: { items: [{ rule: { id: 'dev.go_build' }, foundFiles: 3, foundBytes: 8192n }] },
    maintenance: {
      results: {},
      isStale: vi.fn(() => false),
      refresh: vi.fn().mockResolvedValue(undefined),
      invalidate: vi.fn(),
    },
    busy: false,
    saving: false,
    setHistoryEpoch: vi.fn(),
  } as unknown as AppController;
}
it('shows consequences and cancels without submitting or losing keyboard focus', async () => {
  vi.mocked(api.prepareTool).mockResolvedValue(plan);
  render(<ToolCacheCard model={model()} id="dev.go_build" title={plan.title} />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: plan.title }));
  expect(await screen.findByRole('dialog')).toHaveTextContent('下次编译较慢');
  await user.click(screen.getByRole('button', { name: '取消' }));
  expect(api.executeTool).not.toHaveBeenCalled();
  expect(screen.getByRole('button', { name: plan.title })).toHaveFocus();
});
it('submits once, invalidates only that rule and reconciles a lost response without replay', async () => {
  vi.mocked(api.prepareTool).mockResolvedValue(plan);
  vi.mocked(api.executeTool).mockRejectedValue(new Error('connection lost'));
  const state = model();
  render(<ToolCacheCard model={state} id="dev.go_build" title={plan.title} />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: plan.title }));
  await user.dblClick(await screen.findByRole('button', { name: '确认执行' }));
  expect(api.executeTool).toHaveBeenCalledExactlyOnceWith('one-use');
  expect(state.maintenance.invalidate).toHaveBeenCalledExactlyOnceWith('dev.go_build');
  expect(state.maintenance.refresh).toHaveBeenCalledOnce();
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(screen.getByRole('alert')).toHaveTextContent('connection lost');
});
it('blocks stale and incomplete scans instead of displaying an old actionable amount', () => {
  const state = model();
  vi.mocked(state.maintenance.isStale).mockReturnValue(true);
  render(<ToolCacheCard model={state} id="dev.go_build" title={plan.title} />);
  expect(screen.getByRole('button', { name: plan.title })).toBeDisabled();
  expect(screen.queryByText(/扫描发现/)).not.toBeInTheDocument();
  expect(screen.getByText(/其他选择已保留/)).toBeInTheDocument();
});
it('recovers an existing task without invalidating a later full scan or starting another command', async () => {
  const report = {
    ...plan,
    session: 'first',
    status: 'completed',
    started_at: 1,
    finished_at: 2,
    before_bytes: '1',
    after_bytes: '1',
    free_before: null,
    free_after: null,
    message: '未发现可整理内容',
    output: '',
    command: 'go clean -cache',
  } as ToolReport;
  vi.mocked(api.toolTask).mockResolvedValue(report);
  const hook = renderHook(({ session }) => useToolTask(true, session), {
    initialProps: { session: 'first' },
  });
  await waitFor(() => expect(hook.result.current.isStale('dev.go_build')).toBe(true));
  expect(hook.result.current.isStale('dev.pnpm')).toBe(false);
  hook.rerender({ session: 'new' });
  await act(() => hook.result.current.refresh());
  expect(hook.result.current.isStale('dev.go_build')).toBe(false);
  expect(api.executeTool).not.toHaveBeenCalled();
});
it('distinguishes documented eligibility from real-machine acceptance and leaves unknown apps readonly', () => {
  render(
    <>
      <CleanupEvidence ruleId="windows.crash_dumps" />
      <CleanupEvidence ruleId="baidu_netdisk.http" />
    </>,
  );
  expect(screen.getByText(/非声明所有 .dmp 都是垃圾/)).toBeInTheDocument();
  expect(screen.getByText('尚无足够的清理依据，保持只读。')).toBeInTheDocument();
  expect(screen.getAllByText(/对应清理操作验收：未验证/)).toHaveLength(2);
});

it('does not invalidate a new scan when an older task is first observed after reconnect', async () => {
  vi.mocked(api.toolTask).mockResolvedValue({
    rule_id: 'dev.go_build',
    id: 'old',
    session: 'old-scan',
    status: 'completed',
  } as ToolReport);
  const hook = renderHook(() => useToolTask(true, 'new-scan'));
  await waitFor(() => expect(hook.result.current.task?.id).toBe('old'));
  expect(hook.result.current.isStale('dev.go_build')).toBe(false);
  expect(api.executeTool).not.toHaveBeenCalled();
});
it('offers query recovery without submitting another cleanup', async () => {
  const state = model();
  state.maintenance.error = '读取失败';
  render(<ToolCacheCard model={state} id="dev.go_build" title={plan.title} />);
  await userEvent.click(screen.getByRole('button', { name: '重新查询状态' }));
  expect(state.maintenance.refresh).toHaveBeenCalledOnce();
  expect(api.executeTool).not.toHaveBeenCalled();
});

it('cancels preparation by token and discards a late plan without submitting deletion', async () => {
  let finish!: (plan: ToolPlan) => void;
  vi.mocked(api.prepareTool).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  vi.mocked(api.cancelToolPreparation).mockResolvedValue(undefined);
  render(<ToolCacheCard model={model()} id="dev.go_build" title={plan.title} />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: plan.title }));
  await user.click(screen.getByRole('button', { name: '取消检查' }));
  expect(api.cancelToolPreparation).toHaveBeenCalledWith(
    vi.mocked(api.prepareTool).mock.calls[0][2],
  );
  await act(async () => finish(plan));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(api.executeTool).not.toHaveBeenCalled();
});
