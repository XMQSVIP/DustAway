import type { AppController } from './useAppController';
import { isSelectableCleanup } from './cleanupPresentation';
import { isOfficialRule } from './ToolCache';
import { formatBytes } from './i18n';

export function scanAccounting(model: Pick<AppController, 'decision' | 'summaryRows'>) {
  const items = model.decision.items.filter((item) => !isOfficialRule(item.rule.id));
  const observed = items.reduce((sum, item) => sum + (item.recognizedBytes ?? item.foundBytes), 0n);
  const available = items
    .filter(isSelectableCleanup)
    .reduce((sum, item) => sum + item.foundBytes, 0n);
  const incomplete = model.summaryRows.some(
    (row) =>
      row.scan_failed ||
      row.coverage?.complete === false ||
      ['partial', 'failed', 'interrupted'].includes(row.phase),
  );
  const unknown =
    model.summaryRows.some(
      (row) => !row.observation || row.skipped > row.observation.retained_files,
    ) || incomplete;
  return { observed, available, retained: observed - available, incomplete, unknown };
}
export function ScanTotals({ model }: { model: AppController }) {
  const data = scanAccounting(model);
  const measured = (bytes: bigint) =>
    data.unknown
      ? bytes === 0n
        ? '未知'
        : `${formatBytes(bytes)} + 未知大小`
      : formatBytes(bytes);
  return (
    <div className="scan-accounting">
      <dl aria-label="扫描容量统计">
        <div>
          <dt>已扫描占用</dt>
          <dd>{measured(data.observed)}</dd>
        </div>
        <div>
          <dt>可直接清理</dt>
          <dd>{formatBytes(data.available)}</dd>
        </div>
        <div>
          <dt>已保留</dt>
          <dd>{measured(data.retained)}</dd>
        </div>
      </dl>
      <p>
        {data.incomplete ? '部分位置未读取，以上仅为已读取范围。' : '仅统计本次已读取的文件范围。'}
        Windows 官方估算单独列出；勾选仅改变本次已选量。
      </p>
    </div>
  );
}
