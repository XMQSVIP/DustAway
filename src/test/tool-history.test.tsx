import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { HistoryPage } from '../HistoryPage';
import { UvReceipt, type UvReport } from '../UvCache';
import { api } from '../api';
import type { AppController } from '../useAppController';
vi.mock('../api', () => ({
  desktop: true,
  api: {
    uvHistory: vi.fn(),
    toolHistory: vi.fn(),
    uninstallHistory: vi.fn().mockResolvedValue([]),
  },
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
const report: UvReport = {
  id: 'receipt',
  started_at: 1,
  finished_at: 2,
  status: 'completed',
  cache: 'C:\\fixture',
  command: 'uv cache prune',
  message: '未观察到缓存变化',
  output: '',
  before_bytes: null,
  after_bytes: '0',
  free_before: '1024',
  free_after: null,
};
it('keeps successful uv history when another tool history fails', async () => {
  vi.mocked(api.uvHistory).mockResolvedValue([report]);
  vi.mocked(api.toolHistory).mockRejectedValue(new Error('corrupt tool record'));
  const model = {
    history: [],
    historyEpoch: 0,
    setHistoryEpoch: vi.fn(),
    setClearDialog: vi.fn(),
  } as unknown as AppController;
  render(<HistoryPage model={model} />);
  expect(await screen.findByText('uv 缓存整理 · 已完成')).toBeInTheDocument();
  expect(screen.getByRole('alert')).toHaveTextContent('其他官方工具');
  vi.mocked(api.toolHistory).mockResolvedValue([]);
  await userEvent.setup().click(screen.getByRole('button', { name: '重新读取其他官方工具历史' }));
  expect(api.toolHistory).toHaveBeenCalledTimes(2);
  expect(api.uvHistory).toHaveBeenCalledTimes(1);
  expect(screen.getByText('uv 缓存整理 · 已完成')).toBeInTheDocument();
});
it('shows missing logical and disk readings explicitly without calculating reclaimed bytes', () => {
  render(<UvReceipt report={report} />);
  expect(screen.getByText(/缓存逻辑大小：未取得/)).toBeInTheDocument();
  expect(screen.getByText(/所在磁盘可用空间：.*未取得/)).toBeInTheDocument();
  expect(screen.queryByText(/释放了|已删除/)).not.toBeInTheDocument();
});
