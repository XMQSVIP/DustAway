import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useManualPreview } from '../useManualPreview';
import { useHomeSpace } from '../useHomeSpace';
import type { AnalysisContext, AnalysisLocation, ManualPreview, ManualSelection } from '../types';
const mock = vi.hoisted(() => ({
  manualPreview: vi.fn(),
  cancelManualPreview: vi.fn(),
  analysisContext: vi.fn(),
}));
vi.mock('../api', () => ({ api: mock }));
beforeEach(() => {
  vi.resetAllMocks();
  mock.cancelManualPreview.mockResolvedValue(undefined);
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});
const selection = (id: string): ManualSelection => ({
  session: 'session',
  selected_ids: [id],
  excluded_ids: [],
});
const preview = (
  s: ManualSelection,
  request_id: string,
  bytes = '9007199254740993',
): ManualPreview => ({
  session: s.session,
  request_id,
  bytes,
  files: 1,
  directories: 0,
  retained: 0,
});
it('coalesces changes and ignores out-of-order summaries while cancelling only the obsolete request', async () => {
  vi.useFakeTimers();
  const pending: { s: ManualSelection; id: string; resolve: (p: ManualPreview) => void }[] = [];
  mock.manualPreview.mockImplementation(
    (s, id) => new Promise((resolve) => pending.push({ s, id, resolve })),
  );
  const hook = renderHook(({ s }) => useManualPreview(s), {
    initialProps: { s: selection('one') },
  });
  hook.rerender({ s: selection('two') });
  await act(() => vi.advanceTimersByTimeAsync(300));
  expect(pending).toHaveLength(1);
  expect(pending[0].s.selected_ids).toEqual(['two']);
  hook.rerender({ s: selection('three') });
  expect(hook.result.current.loading).toBe(true);
  expect(hook.result.current.data).toBeNull();
  await act(() => vi.advanceTimersByTimeAsync(300));
  expect(mock.cancelManualPreview).toHaveBeenCalledWith(pending[0].id);
  await act(async () => pending[1].resolve(preview(pending[1].s, pending[1].id)));
  await act(async () => pending[0].resolve(preview(pending[0].s, pending[0].id, '1')));
  expect(hook.result.current.data?.bytes).toBe('9007199254740993');
});
it('keeps failures unknown and retries; rejects a summary for another selection request', async () => {
  vi.useFakeTimers();
  mock.manualPreview.mockImplementationOnce(async (s) => preview(s, 'wrong'));
  const s = selection('empty-directory');
  const hook = renderHook(() => useManualPreview(s));
  await act(() => vi.advanceTimersByTimeAsync(300));
  expect(hook.result.current.error).toBeTruthy();
  expect(hook.result.current.data).toBeNull();
  mock.manualPreview.mockImplementationOnce(async (s, id) => ({
    ...preview(s, id, '0'),
    files: 0,
    directories: 1,
  }));
  act(() => hook.result.current.retry());
  await act(() => vi.advanceTimersByTimeAsync(300));
  expect(hook.result.current.data).toMatchObject({ bytes: '0', files: 0, directories: 1 });
});
const disks: AnalysisLocation[] = ['C', 'D', 'E', 'F'].map((letter) => ({
  token: letter,
  path: `${letter}:\\`,
  title: letter,
  kind: 'volume',
  supported: true,
}));
const context = (token: string, bytes = '100'): AnalysisContext => ({
  location_token: token,
  volume: `${token}:\\`,
  volume_key: token,
  available_bytes: bytes,
  sampled_at: 1,
  error: null,
});
it('retains a completed summary across preparation and cancelled confirmation without another request', async () => {
  vi.useFakeTimers();
  mock.manualPreview.mockImplementation(async (s, id) => preview(s, id));
  const s = selection('one');
  const hook = renderHook(({ paused }) => useManualPreview(s, paused), {
    initialProps: { paused: false },
  });
  await act(() => vi.advanceTimersByTimeAsync(300));
  hook.rerender({ paused: true });
  expect(hook.result.current.data?.bytes).toBe('9007199254740993');
  hook.rerender({ paused: false });
  await act(() => vi.advanceTimersByTimeAsync(3000));
  expect(mock.manualPreview).toHaveBeenCalledTimes(1);
  expect(mock.cancelManualPreview).not.toHaveBeenCalled();
});
it('shows elapsed time after two seconds and resumes only an unfinished summary after preparation', async () => {
  vi.useFakeTimers();
  mock.manualPreview.mockImplementation(() => new Promise(() => {}));
  const s = selection('one');
  const hook = renderHook(({ paused }) => useManualPreview(s, paused), {
    initialProps: { paused: false },
  });
  await act(() => vi.advanceTimersByTimeAsync(1900));
  expect(hook.result.current.elapsed).toBe(0);
  await act(() => vi.advanceTimersByTimeAsync(1100));
  expect(hook.result.current.elapsed).toBe(3);
  hook.rerender({ paused: true });
  expect(mock.cancelManualPreview).toHaveBeenCalledTimes(1);
  mock.manualPreview.mockImplementation(async (s, id) => preview(s, id, '0'));
  hook.rerender({ paused: false });
  await act(() => vi.advanceTimersByTimeAsync(300));
  expect(mock.manualPreview).toHaveBeenCalledTimes(2);
  expect(hook.result.current.data?.bytes).toBe('0');
});
it('samples selected disks separately with two concurrent requests even across selection changes', async () => {
  const pending: { token: string; resolve: (value: AnalysisContext) => void }[] = [];
  mock.analysisContext.mockImplementation(
    (token) => new Promise((resolve) => pending.push({ token, resolve })),
  );
  const hook = renderHook(({ selected }) => useHomeSpace(disks, selected, ''), {
    initialProps: { selected: null as string[] | null },
  });
  expect(pending.map((p) => p.token)).toEqual(['C', 'D']);
  hook.rerender({ selected: ['F'] });
  expect(pending).toHaveLength(2);
  await act(async () => pending[0].resolve(context('C')));
  expect(pending.map((p) => p.token)).toEqual(['C', 'D', 'F']);
  await act(async () => pending[2].resolve(context('F', '200')));
  await act(async () => pending[1].resolve(context('D')));
  expect(hook.result.current.volumes.map((v) => v.token)).toEqual(['F']);
  expect([...hook.result.current.readings.keys()]).toEqual(['F']);
  expect(hook.result.current.readings.get('F')?.context?.available_bytes).toBe('200');
});
it('keeps the last reading on a per-disk failure, refreshes on completion and rejects wrong volume tokens', async () => {
  mock.analysisContext.mockImplementation(async (token) => context(token));
  const hook = renderHook(({ report }) => useHomeSpace(disks, ['C', 'D'], report), {
    initialProps: { report: '' },
  });
  await waitFor(() => expect(hook.result.current.readings.get('D')?.loading).toBe(false));
  mock.analysisContext.mockImplementation(async (token) =>
    token === 'C' ? context('D') : context('D', '300'),
  );
  hook.rerender({ report: 'completed' });
  await waitFor(() => expect(hook.result.current.readings.get('C')?.error).toBeTruthy());
  expect(hook.result.current.readings.get('C')?.context?.available_bytes).toBe('100');
  expect(hook.result.current.readings.get('D')?.context?.available_bytes).toBe('300');
  mock.analysisContext.mockImplementation(async (token) => context(token, '400'));
  act(() => hook.result.current.refresh());
  await waitFor(() =>
    expect(hook.result.current.readings.get('C')?.context?.available_bytes).toBe('400'),
  );
  expect(mock.analysisContext).toHaveBeenCalledTimes(6);
});
