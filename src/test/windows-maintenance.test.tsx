import { cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { useWindowsMaintenance } from '../WindowsMaintenance';
const mock = vi.hoisted(() => ({ windowsSnapshot: vi.fn() }));
vi.mock('../api', () => ({ desktop: true, api: mock }));
afterEach(cleanup);
it('reports a disconnected running operation as uncertain without retrying deletion', async () => {
  mock.windowsSnapshot.mockResolvedValueOnce({
    session: 'one',
    status: 'running',
    actions: [],
    plan: null,
    report: null,
    current: null,
    message: '',
  });
  mock.windowsSnapshot.mockRejectedValue(new Error('pipe closed'));
  const { result } = renderHook(() => useWindowsMaintenance(true, false));
  await waitFor(() => expect(result.current.busy).toBe(true));
  await waitFor(() => expect(result.current.snapshots.user?.status).toBe('uncertain'), {
    timeout: 2500,
  });
  expect(result.current.busy).toBe(false);
  expect(result.current.plans).toEqual([]);
  expect(result.current.snapshots.user?.message).toContain('不会自动重试删除');
});
