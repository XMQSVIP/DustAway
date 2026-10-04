import { useEffect, useRef, useState } from 'react';
import { api } from './api';
import { ipcError } from './coordinator';
import type { ManualPreview, ManualSelection } from './types';
import { useElapsed } from './useElapsed';

export function useManualPreview(selection: ManualSelection | undefined, paused = false) {
  const [epoch, setEpoch] = useState(0);
  const [state, setState] = useState<{
    selection?: ManualSelection;
    data: ManualPreview | null;
    error: string;
    loading: boolean;
  }>({ data: null, error: '', loading: false });
  const stop = useRef<() => void>(() => {});
  const completed = useRef<{
    selection: ManualSelection;
    data: ManualPreview;
    epoch: number;
  } | null>(null);
  useEffect(() => {
    if (!selection) return;
    if (completed.current?.selection === selection && completed.current.epoch === epoch) {
      setState({ selection, data: completed.current.data, error: '', loading: false });
      return;
    }
    completed.current = null;
    if (paused) return;
    const requestId = crypto.randomUUID();
    let valid = true,
      started = false,
      finished = false;
    setState({ selection, data: null, error: '', loading: true });
    const timer = setTimeout(() => {
      started = true;
      void api
        .manualPreview(selection, requestId)
        .then((data) => {
          if (!valid) return;
          if (data.request_id !== requestId || data.session !== selection.session)
            throw new Error('已选统计已变化，请重试');
          completed.current = { selection, data, epoch };
          finished = true;
          setState({ selection, data, error: '', loading: false });
        })
        .catch((error) => {
          finished = true;
          if (valid)
            setState({ selection, data: null, error: ipcError(error).message, loading: false });
        });
    }, 300);
    const cancel = () => {
      valid = false;
      clearTimeout(timer);
      if (started && !finished) void api.cancelManualPreview(requestId).catch(() => {});
    };
    stop.current = cancel;
    return cancel;
  }, [selection, epoch, paused]);
  const current = !!selection && state.selection === selection;
  const loading = !!selection && !paused && (!current || state.loading);
  const elapsed = useElapsed(loading, selection);
  return {
    data: current ? state.data : null,
    error: current ? state.error : '',
    loading,
    elapsed,
    retry: () => setEpoch((n) => n + 1),
    cancel: () => stop.current(),
  };
}
