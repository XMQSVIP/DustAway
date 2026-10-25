import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { WeixinCleanup } from '../WeixinCleanup';
import { cleanupDecision } from '../cleanupDecision';
import rules from '../previewRules';
import type { AppController } from '../useAppController';
import type { RuleSummary } from '../types';
vi.mock('../api', () => ({ desktop: true, api: {} }));
vi.mock('../CleanupRuleList', () => ({ CleanupRuleList: () => <div>专项文件列表</div> }));
afterEach(cleanup);
function model(rows: RuleSummary[] = [], scanning = false) {
  return {
    summaryRows: rows,
    busy: false,
    saving: false,
    canScanVolumes: true,
    scan: vi.fn(),
    refreshAvailability: vi.fn(),
    decision: cleanupDecision({
      rules: rules
        .filter((rule) => rule.id === 'weixin.logs')
        .map((rule) => ({ ...rule, cleanup_validated: false })),
      rows,
      selected: new Set(),
      excluded: new Map(),
      sessions: rows.length ? [{ scope: 'user', id: 's' }] : [],
      reports: [],
      canPlan: true,
      needsRescan: false,
      uncertain: false,
      stopping: false,
      active: scanning ? 'scanning' : null,
    }),
  } as unknown as AppController;
}
const recent: RuleSummary = {
  rule_id: 'weixin.logs',
  phase: 'completed',
  files: 0,
  bytes: '0',
  skipped: 17,
  blocked: true,
  note: null,
  scan_failed: false,
  scan_issue: null,
  availability: 'validation_pending',
  reasons: [{ code: 'recent', count: 17 }],
  coverage: { complete: true, issues: [] },
  observation: {
    retained_files: 17,
    retained_bytes: '118852485',
    recent_files: 17,
    recent_bytes: '118852485',
    application: null,
  },
};
it('keeps Weixin visible before scanning and does not present unknown occupancy as zero', () => {
  const value = model();
  const attachments = vi.fn();
  render(<WeixinCleanup model={value} onAttachments={attachments} />);
  expect(within(screen.getByLabelText('微信缓存与日志容量')).getAllByText('待扫描')).toHaveLength(
    3,
  );
  expect(screen.getByText('专项文件列表')).not.toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: '微信专项清理' }));
  expect(screen.getByText('专项文件列表')).toBeVisible();
  expect(value.scan).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '整理微信聊天附件' }));
  expect(attachments).toHaveBeenCalledOnce();
});
it('keeps incomplete scan status visible when collapsed and distinguishes it from an empty scan', () => {
  render(
    <WeixinCleanup
      model={model([
        { ...recent, files: 0, skipped: 1, phase: 'failed', scan_failed: true, observation: null },
      ])}
      onAttachments={vi.fn()}
    />,
  );
  expect(screen.getByText('扫描未完整结束')).toBeVisible();
  expect(screen.getByText(/部分微信位置尚未读完/)).toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: '微信专项清理' }));
  expect(screen.getAllByText('未知大小')).toHaveLength(2);
  expect(screen.getByText('待核验')).toBeVisible();
  expect(screen.getByText(/部分微信位置尚未读完/)).toBeVisible();
  expect(screen.queryByText('0 B')).not.toBeInTheDocument();
});
it('keeps pending rows unmeasured and exposes readonly reasons when expanded', () => {
  const value = model([{ ...recent, phase: 'pending', observation: null }], true);
  const { rerender } = render(<WeixinCleanup model={value} onAttachments={vi.fn()} />);
  expect(screen.getByText('扫描中')).toBeVisible();
  expect(screen.queryByText('0 B')).not.toBeInTheDocument();
  rerender(
    <WeixinCleanup
      model={model([
        {
          ...recent,
          files: 2,
          bytes: '4096',
          reasons: [],
          observation: { ...recent.observation!, retained_files: 0, retained_bytes: '0' },
        },
      ])}
      onAttachments={vi.fn()}
    />,
  );
  expect(screen.getByText('当前缓存与日志仅供查看')).toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: '微信专项清理' }));
  expect(screen.getByText(/主要保留原因：尚未开放清理/)).toBeVisible();
});
it('separates runtime from readonly permission and refreshes without rescanning or selecting', () => {
  const value = model([recent]);
  value.applicationInfo = {
    loading: false,
    error: '',
    applications: [
      {
        app_id: 'weixin',
        identity_verified: true,
        running_components: ['Weixin.exe'],
        status: null,
      } as never,
    ],
  };
  const { rerender } = render(<WeixinCleanup model={value} onAttachments={vi.fn()} />);
  expect(screen.getByText('微信或后台组件正在运行')).toBeVisible();
  expect(screen.getByText('当前缓存与日志仅供查看')).toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: '微信专项清理' }));
  fireEvent.click(screen.getByRole('button', { name: '重新检查微信运行状态' }));
  expect(value.refreshAvailability).toHaveBeenCalledOnce();
  expect(value.scan).not.toHaveBeenCalled();
  expect(screen.getByText(/不查找新增文件/)).toBeVisible();
  value.applicationInfo.applications[0].running_components = [];
  value.applicationInfo.applications[0].rules = [
    { rule_id: 'weixin.logs', cleanup_validated: false, cleanup_block: 'process_unknown' },
  ];
  rerender(<WeixinCleanup model={value} onAttachments={vi.fn()} />);
  expect(screen.getByText('尚未确认')).toBeVisible();
  expect(screen.queryByText('未检测到微信进程')).not.toBeInTheDocument();
});
it('shows recent-only Weixin occupancy independently of other apps and selection', () => {
  render(
    <WeixinCleanup
      model={model([
        recent,
        {
          ...recent,
          rule_id: 'qq.http',
          observation: { ...recent.observation!, retained_bytes: '9000000000' },
        },
      ])}
      onAttachments={vi.fn()}
    />,
  );
  const totals = within(screen.getByLabelText('微信缓存与日志容量'));
  expect(totals.getAllByText('113.3 MB')).toHaveLength(2);
  expect(totals.queryByText(/GB/)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '微信专项清理' }));
  expect(screen.getByText(/聊天图片、视频和文件需选择账号后单独整理/)).toBeVisible();
});
it('marks incomplete occupancy and keeps unavailable controls disabled during work', () => {
  const value = model([{ ...recent, phase: 'partial', coverage: { complete: false, issues: [] } }]);
  value.busy = true;
  render(<WeixinCleanup model={value} onAttachments={vi.fn()} />);
  expect(screen.getAllByText(/113.3 MB \+ 未知大小/)).toHaveLength(2);
  expect(screen.getByRole('button', { name: '整理微信聊天附件' })).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: '微信专项清理' }));
  expect(screen.getByText(/部分微信位置尚未读完/)).toBeVisible();
  expect(screen.getByRole('button', { name: '重新常规扫描' })).toBeDisabled();
});
