import type { RuleOutcome } from './types';
import { formatResultCount, unprocessed, type ResultStatus } from './cleanupResults';
export { fullyDeleted, resultTitle, unprocessed } from './cleanupResults';
export function ResultCounts({
  result,
  onDetail,
  unit = '个文件',
}: {
  result: Pick<RuleOutcome, 'total' | 'deleted' | 'skipped' | 'failed' | 'unknown'>;
  onDetail?: (status: ResultStatus) => void;
  unit?: '个文件' | '个文件夹';
}) {
  const counts = [
    ['deleted', '已删除', result.deleted],
    ['skipped', '跳过未删除', result.skipped],
    ['failed', '失败', result.failed],
    ['unknown', '状态不确定', result.unknown],
    ['candidate', '未处理', unprocessed(result)],
  ] as const;
  return (
    <div className="result-counts">
      {counts.map(([status, label, count]) =>
        onDetail && Number.isSafeInteger(count) && count > 0 ? (
          <button key={status} className={`result-link ${status}`} onClick={() => onDetail(status)}>
            {label} {formatResultCount(count)} {unit}
          </button>
        ) : (
          <span key={status}>
            {label} {formatResultCount(count)} {unit}
          </span>
        ),
      )}
    </div>
  );
}
