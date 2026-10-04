import { useRef, useState } from 'react';
import { api } from './api';
import type { Settings, SaveResult } from './types';
export function useSettings(
  onSaved: (settings: Settings, invalidated: boolean) => void,
  onError: (message: string) => void,
  onUncertain?: () => Promise<void>,
) {
  const [saving, setSaving] = useState(false);
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const count = useRef(0);
  const callback = useRef(onSaved);
  callback.current = onSaved;
  function run(operation: () => Promise<SaveResult>): Promise<SaveResult | undefined> {
    count.current++;
    setSaving(true);
    const result = queue.current.then(async () => {
      try {
        const value = await operation();
        callback.current(value.settings, value.scan_invalidated);
        return value;
      } catch (e) {
        onError(String(e));
        try {
          await onUncertain?.();
        } catch (failure) {
          onError(`设置保存结果暂时无法确认：${String(failure)}`);
        }
        return undefined;
      } finally {
        count.current--;
        setSaving(count.current > 0);
      }
    });
    queue.current = result;
    return result;
  }
  return { saving, run, save: (patch: Partial<Settings>) => run(() => api.save(patch)) };
}
