import { act, cleanup, render, renderHook, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { RuleRefreshNotice } from '../RuleRefresh';
import { ToolCacheCard, ToolCacheCards, useToolTask, type ToolReport } from '../ToolCache';
import { OfficialCacheTools, useUvTask } from '../UvCache';
import { api } from '../api';
import type { AppController } from '../useAppController';
import type { RuleSummary } from '../types';
vi.mock('../api', () => ({
  desktop: true,
  api: { toolTask: vi.fn(), uvTask: vi.fn(), prepareTool: vi.fn(), executeTool: vi.fn() },
}));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
const row = {
  rule_id: 'dev.go_build',
  revision: 1,
  stale: false,
  files: 2,
  bytes: '24',
  skipped: 0,
  blocked: false,
  note: null,
  scan_issue: null,
  availability: null,
  reasons: [],
  phase: 'completed',
  scan_failed: false,
  coverage: { complete: true, issues: [] },
  tool: {
    executable: null,
    entry: null,
    version: null,
    target: null,
    identity: null,
    reason: 'target_unknown',
    ready: false,
    message: '实际处理位置未确认，请重新检查这一项',
  },
} as RuleSummary;
function model() {
  return {
    sessions: [{ scope: 'user', id: 'scan' }],
    summaryRows: [row],
    decision: { items: [{ rule: { id: row.rule_id }, foundFiles: 2, foundBytes: 24n }] },
    maintenance: { results: {}, isStale: () => false },
    refreshRule: vi.fn(),
    cancelRuleRefresh: vi.fn(),
    retryStatus: vi.fn(),
  } as unknown as AppController;
}
it('shows the discovery failure before preparation and offers a read-only retry', async () => {
  const state = model();
  render(<ToolCacheCard model={state} id={row.rule_id} title="清空 Go 构建缓存" />);
  expect(screen.getByRole('button', { name: '清空 Go 构建缓存' })).toBeDisabled();
  expect(screen.getByText(/实际处理位置未确认/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: '重新检查这一项' }));
  expect(state.refreshRule).toHaveBeenCalledWith(row.rule_id);
  expect(api.prepareTool).not.toHaveBeenCalled();
});
it('keeps zero-file query failures visible while hiding tools that were not found', async () => {
  const state = model();
  state.decision.items = [];
  state.summaryRows = [
    {
      ...row,
      files: 0,
      tool: {
        executable: null,
        version: null,
        target: null,
        identity: null,
        ready: false,
        reason: 'target_unknown',
        message: '实际处理位置未确认：查询超时',
      },
    },
  ];
  const view = render(<ToolCacheCards model={state} />);
  expect(screen.getByText(/查询超时/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: '重新检查这一项' }));
  expect(state.refreshRule).toHaveBeenCalledWith(row.rule_id);
  expect(api.prepareTool).not.toHaveBeenCalled();
  view.rerender(
    <ToolCacheCards
      model={{
        ...state,
        summaryRows: [
          {
            ...row,
            files: 0,
            tool: {
              executable: null,
              version: null,
              target: null,
              identity: null,
              ready: false,
              reason: 'tool_missing',
              message: '未找到工具',
            },
          },
        ],
      }}
    />,
  );
  expect(screen.queryByText('未找到工具')).not.toBeInTheDocument();
  view.rerender(
    <ToolCacheCards
      model={{
        ...state,
        summaryRows: [
          {
            ...state.summaryRows[0],
            tool: {
              ...state.summaryRows[0].tool!,
              executable: 'C:\\Tools\\go.exe',
              reason: 'cache_missing',
              message: '当前位置没有缓存',
            },
          },
        ],
      }}
    />,
  );
  expect(screen.queryByText('当前位置没有缓存')).not.toBeInTheDocument();
});
it('shows inline cancellation, retains failure, and restores focus when the cancel button disappears', async () => {
  const state = model();
  state.ruleRefresh = {
    id: 'refresh',
    rule_id: row.rule_id,
    revision: 1,
    state: 'scanning',
    progress: { checked: 20, elapsed_ms: 1, last_progress_ms: 1 },
    error: null,
  };
  const view = render(<RuleRefreshNotice model={state} rule={row.rule_id} />);
  const cancel = screen.getByRole('button', { name: '取消此项检查' });
  await userEvent.click(cancel);
  expect(state.cancelRuleRefresh).toHaveBeenCalledOnce();
  view.rerender(
    <RuleRefreshNotice
      model={{
        ...state,
        ruleRefresh: {
          ...state.ruleRefresh,
          state: 'cancelled',
          error: '检查已取消；旧结果已保留',
        },
      }}
      rule={row.rule_id}
    />,
  );
  expect(screen.getByRole('alert')).toHaveTextContent('旧结果已保留');
  expect(screen.getByLabelText('此项检查状态')).toHaveFocus();
});
it('hides empty cache cards and notices after refresh and returns focus to results', async () => {
  const state = model();
  state.decision.items = [];
  state.summaryRows = [{ ...row, rule_id: 'dev.uv', files: 0, stale: true }];
  state.ruleRefresh = {
    id: 'refresh',
    rule_id: 'dev.uv',
    revision: 1,
    state: 'scanning',
    progress: { checked: 0, elapsed_ms: 0, last_progress_ms: 0 },
    error: null,
  };
  const view = render(
    <div className="desk-results" tabIndex={-1}>
      <OfficialCacheTools model={state} />
    </div>,
  );
  const cancel = screen.getByRole('button', { name: '取消此项检查' });
  cancel.focus();
  view.rerender(
    <div className="desk-results" tabIndex={-1}>
      <OfficialCacheTools
        model={{
          ...state,
          summaryRows: [
            {
              ...state.summaryRows[0],
              stale: false,
              tool: {
                executable: null,
                version: null,
                target: null,
                identity: null,
                ready: false,
                reason: 'cache_missing',
                message: 'uv 缓存目录为空，无需整理',
              },
            },
          ],
          ruleRefresh: { ...state.ruleRefresh, state: 'finished' },
        }}
      />
    </div>,
  );
  await waitFor(() => expect(document.querySelector('.desk-results')).toHaveFocus());
  expect(screen.queryByRole('heading', { name: 'uv 闲置缓存' })).not.toBeInTheDocument();
  expect(screen.queryByText('官方工具整理')).not.toBeInTheDocument();
  expect(screen.queryByRole('status')).not.toBeInTheDocument();
});
it.each(['unsupported_layout', 'process_unknown', 'target_unknown'])(
  'does not claim no files when a zero-file refresh failed with %s',
  (reason) => {
    const state = model();
    state.summaryRows = [
      { ...row, files: 0, tool: { ...row.tool!, reason, message: '无法核验缓存位置' } },
    ];
    state.ruleRefresh = {
      id: 'finished-check',
      rule_id: row.rule_id,
      revision: 2,
      state: 'finished',
      progress: { checked: 0, elapsed_ms: 1, last_progress_ms: 1 },
      error: null,
    };
    render(<RuleRefreshNotice model={state} rule={row.rule_id} />);
    expect(screen.getByText('无法核验缓存位置')).toBeVisible();
    expect(screen.getByRole('status')).toHaveTextContent('检查未通过，暂不能确认可整理内容');
    expect(screen.queryByText(/没有符合当前规则的文件/)).not.toBeInTheDocument();
  },
);
it('does not report an empty cache from a partial directory read', () => {
  const state = model();
  state.summaryRows = [
    {
      ...row,
      files: 0,
      tool: { ...row.tool!, ready: true },
      coverage: { complete: false, issues: [] },
    },
  ];
  state.ruleRefresh = {
    id: 'partial-check',
    rule_id: row.rule_id,
    revision: 2,
    state: 'finished',
    progress: { checked: 0, elapsed_ms: 1, last_progress_ms: 1 },
    error: null,
  };
  render(<RuleRefreshNotice model={state} rule={row.rule_id} />);
  expect(screen.getByRole('status')).toHaveTextContent('检查未通过');
  expect(screen.queryByText(/没有符合当前规则的文件/)).not.toBeInTheDocument();
});
it('keeps incomplete zero-file uv inspections visible instead of treating them as empty', () => {
  const state = model();
  state.decision.items = [];
  state.summaryRows = [
    {
      ...row,
      rule_id: 'dev.uv',
      files: 0,
      phase: 'partial',
      tool: null,
      coverage: { complete: false, issues: [] },
    },
  ];
  render(<OfficialCacheTools model={state} />);
  expect(screen.getByRole('heading', { name: 'uv 闲置缓存' })).toBeVisible();
  expect(screen.getByRole('button', { name: '重新检查这一项' })).toBeEnabled();
});
it('does not let a late task invalidate a newer revision and keeps old reports out of a new scan', async () => {
  const report = {
    id: 'old-task',
    rule_id: row.rule_id,
    session: 'scan',
    status: 'completed',
  } as ToolReport;
  vi.mocked(api.toolTask).mockResolvedValue(report);
  const hook = renderHook(
    ({ revision, session }) => useToolTask(true, session, [{ ...row, revision }]),
    { initialProps: { revision: 1, session: 'scan' } },
  );
  act(() => hook.result.current.invalidate(row.rule_id));
  expect(hook.result.current.isStale(row.rule_id)).toBe(true);
  hook.rerender({ revision: 2, session: 'scan' });
  await waitFor(() => expect(hook.result.current.task?.id).toBe('old-task'));
  expect(hook.result.current.isStale(row.rule_id)).toBe(false);
  hook.rerender({ revision: 1, session: 'new-scan' });
  expect(hook.result.current.results).toEqual({});
  expect(api.executeTool).not.toHaveBeenCalled();
});
it('uses backend revisions for uv invalidation and hides earlier task-only cards', async () => {
  vi.mocked(api.uvTask).mockResolvedValue({
    id: 'old-uv',
    session: 'scan',
    status: 'completed',
  } as never);
  const hook = renderHook(
    ({ revision }) => useUvTask(true, 'scan', { ...row, rule_id: 'dev.uv', revision }),
    { initialProps: { revision: 1 } },
  );
  act(() => hook.result.current.invalidate('scan'));
  hook.rerender({ revision: 2 });
  await waitFor(() => expect(hook.result.current.task?.id).toBe('old-uv'));
  expect(hook.result.current.stale).toBe(false);
  const state = model();
  render(
    <OfficialCacheTools
      model={{
        ...state,
        sessions: [{ scope: 'user', id: 'new-scan' }],
        summaryRows: [],
        decision: { ...state.decision, items: [] },
        uv: hook.result.current,
      }}
    />,
  );
  expect(screen.queryByText('官方工具整理')).not.toBeInTheDocument();
});
