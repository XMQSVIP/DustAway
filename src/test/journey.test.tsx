import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useSpaceJourney } from '../useSpaceJourney';
const mock = vi.hoisted(() => ({ analysisLocations: vi.fn(), analysisContext: vi.fn() }));
vi.mock('../api', () => ({ desktop: true, api: mock }));
afterEach(cleanup);
beforeEach(() => {
  vi.clearAllMocks();
  mock.analysisLocations.mockResolvedValue([
    { token: 'system', title: '系统盘', path: 'C:\\', is_system: true, supported: true },
  ]);
  mock.analysisContext.mockResolvedValue({
    location_token: 'system',
    volume: 'C:\\',
    volume_key: 'same-volume',
    available_bytes: '1000',
    sampled_at: 1,
    error: null,
  });
});
it('samples after initialization and once for each terminal report, without polling or sampling on rescan', async () => {
  const hook = renderHook(({ enabled, report }) => useSpaceJourney(enabled, null, report), {
    initialProps: { enabled: false, report: '' },
  });
  expect(mock.analysisLocations).not.toHaveBeenCalled();
  hook.rerender({ enabled: true, report: '' });
  await waitFor(() => expect(hook.result.current.context?.available_bytes).toBe('1000'));
  expect(mock.analysisContext).toHaveBeenCalledTimes(1);
  hook.rerender({ enabled: true, report: 'plan-1:finished:2' });
  await waitFor(() => expect(mock.analysisContext).toHaveBeenCalledTimes(2));
  hook.rerender({ enabled: true, report: 'plan-1:finished:2' });
  hook.rerender({ enabled: true, report: '' });
  expect(mock.analysisContext).toHaveBeenCalledTimes(2);
  hook.rerender({ enabled: true, report: 'plan-2:cancelled:3' });
  await waitFor(() => expect(mock.analysisContext).toHaveBeenCalledTimes(3));
  await act(async () => hook.result.current.refresh());
  await waitFor(() => expect(mock.analysisContext).toHaveBeenCalledTimes(4));
});
it('retains the dated successful reading when refreshing after a report fails', async () => {
  const hook = renderHook(({ report }) => useSpaceJourney(true, null, report), {
    initialProps: { report: '' },
  });
  await waitFor(() => expect(hook.result.current.context?.available_bytes).toBe('1000'));
  mock.analysisContext.mockRejectedValueOnce(new Error('disk unavailable'));
  hook.rerender({ report: 'report:finished' });
  await waitFor(() => expect(hook.result.current.contextError).toContain('disk unavailable'));
  expect(hook.result.current.context?.sampled_at).toBe(1);
  expect(hook.result.current.context?.available_bytes).toBe('1000');
});
