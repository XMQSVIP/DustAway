import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { cleanupResultSummary } from '../cleanupResults';
import { cleanupPresentation } from '../cleanupPresentation';
import { HistoryPage } from '../HistoryPage';
import type { AppController } from '../useAppController';
import type { Report } from '../types';
afterEach(cleanup);
it.each([
  [{ total: 1, deleted: 1, skipped: 0, failed: 0, unknown: 0 }, '清理完成', 'complete'],
  [{ total: 1, deleted: 0, skipped: 0, failed: 0, unknown: 0 }, '清理已结束', 'attention'],
  [{ total: 1, deleted: 0, skipped: 0, failed: 0, unknown: 1 }, '部分结果尚未确认', 'attention'],
  [{ total: 1, deleted: 2, skipped: 0, failed: 0, unknown: 0 }, '部分结果尚未确认', 'attention'],
] as const)(
  'renders the same decision in the current page and historical receipt: %j',
  (directories, title, stage) => {
    const report: Report = {
      id: 'synthetic',
      scope: 'user',
      status: 'finished',
      started_at: 1,
      finished_at: 2,
      total: 0,
      deleted: 0,
      skipped: 0,
      failed: 0,
      unknown: 0,
      deleted_bytes: '0',
      candidate_bytes: '0',
      free_space_change: null,
      categories: [],
      directories,
    };
    const model = {
      history: [report],
      reports: [report],
      hasManual: false,
      summaryRows: [],
      decision: { result: cleanupResultSummary([report]), items: [] },
      setClearDialog: vi.fn(),
    } as unknown as AppController;
    expect(cleanupPresentation(model).stage).toBe(stage);
    render(<HistoryPage model={model} />);
    expect(screen.getAllByText(title)).toHaveLength(2);
    expect(screen.getByText('文件夹')).toBeInTheDocument();
    if (directories.deleted > directories.total)
      expect(screen.getAllByText(/统计不可用/).length).toBeGreaterThan(0);
  },
);
