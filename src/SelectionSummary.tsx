import { formatBytes } from './i18n';
import type { AppController } from './useAppController';
import type { ScopedPlan } from './types';

export function formatSelectionCounts(files: number, directories: number) {
  return `${files.toLocaleString()} 个文件 · ${directories.toLocaleString()} 个文件夹`;
}

/** Frozen plans, not scan estimates, are the source of confirmation totals. */
export function preparedSummary(plans: ScopedPlan[]) {
  let files = 0,
    directories = 0,
    bytes = 0n,
    ruleFiles = 0,
    ruleBytes = 0n;
  for (const { plan } of plans) {
    files += plan.files;
    directories += plan.manual?.directories ?? 0;
    bytes += BigInt(plan.bytes);
    ruleFiles += plan.files - (plan.manual?.files ?? 0);
    ruleBytes += BigInt(plan.bytes) - BigInt(plan.manual?.bytes ?? '0');
  }
  return {
    files,
    directories,
    bytes,
    ruleFiles,
    ruleBytes,
    amount: formatBytes(bytes),
    counts: formatSelectionCounts(files, directories),
  };
}

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
      ? `${formatSelectionCounts(estimate.files, estimate.directories)}${model.windowsMaintenance?.selected.size ? ` · Windows 维护 ${model.windowsMaintenance.selected.size} 项另计` : ''}`
      : manualPreview.elapsed
        ? `选择已保留 · 已用 ${manualPreview.elapsed} 秒`
        : '选择已保留',
  };
}
