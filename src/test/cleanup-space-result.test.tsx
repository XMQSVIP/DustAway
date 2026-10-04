import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { CleanupSpaceResult } from '../CleanupSpaceResult';
import type { AppController } from '../useAppController';
afterEach(cleanup);
it('shows actual per-volume samples even when available space decreases', () => {
  const model = {
    reports: [
      {
        id: 'report',
        deleted_bytes: '999999',
        volume_space_changes: [
          {
            volume: 'C:\\',
            before_bytes: '2048',
            after_bytes: '1024',
            change_bytes: '-1024',
            error: null,
          },
          {
            volume: 'D:\\',
            before_bytes: null,
            after_bytes: null,
            change_bytes: null,
            error: 'unavailable',
          },
        ],
      },
    ],
    homeSpace: { volumes: [], readings: new Map(), refresh: vi.fn() },
  } as unknown as AppController;
  render(<CleanupSpaceResult model={model} />);
  expect(screen.getByText(/C:.*清理前/)).toHaveTextContent('清理前 2.0 KB → 清理后 1.0 KB');
  expect(screen.getByText(/D:.*清理前/)).toHaveTextContent('清理前 未取得 → 清理后 未取得');
  expect(screen.queryByText(/释放/)).not.toBeInTheDocument();
});
