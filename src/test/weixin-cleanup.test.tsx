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
function model(rows: RuleSummary[] = []) {
  return {
    summaryRows: rows,
    busy: false,
    saving: false,
    canScanVolumes: true,
    scan: vi.fn(),
    refreshAvailability: vi.fn(),
    decision: cleanupDecision({
      rules: rules.filter((rule) => rule.id === 'weixin.logs'),
      rows,
      selected: new Set(),
      excluded: new Map(),
      sessions: rows.length ? [{ scope: 'user', id: 's' }] : [],
      reports: [],
      canPlan: true,
      needsRescan: false,
      uncertain: false,
      stopping: false,
      active: null,
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
  expect(screen.getAllByText('待扫描')).toHaveLength(3);
  expect(screen.getByText('专项文件列表')).not.toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: '微信专项清理' }));
  expect(screen.getByText('专项文件列表')).toBeVisible();
  expect(value.scan).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '整理微信聊天附件' }));
  expect(attachments).toHaveBeenCalledOnce();
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
