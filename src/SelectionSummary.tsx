import { formatBytes } from './i18n';
import type { AppController } from './useAppController';

/** One source for header, desktop sidebar and compact task dock. */
export function selectionSummary(
  model: Pick<AppController, 'selectionEstimate' | 'manualPreview'> &
    Partial<Pick<AppController, 'windowsMaintenance'>>,
) {
  const { selectionEstimate: estimate, manualPreview } = model;
  return {
    amount: estimate.known
      ? formatBytes(estimate.bytes)
      : manualPreview.error
        ? '总量暂不可用'
        : '正在统计…',
    counts: estimate.known
      ? `${estimate.files.toLocaleString()} 个文件 · ${estimate.directories.toLocaleString()} 个文件夹${model.windowsMaintenance?.selected.size ? ` · Windows 维护 ${model.windowsMaintenance.selected.size} 项另计` : ''}`
      : manualPreview.elapsed
        ? `选择已保留 · 已用 ${manualPreview.elapsed} 秒`
        : '选择已保留',
  };
}
