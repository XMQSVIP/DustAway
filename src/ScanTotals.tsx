import type { AppController } from './useAppController';
import { scanAccountingFor, scanAmount } from './scanAssessment';
export function scanAccounting(model: Pick<AppController, 'decision' | 'summaryRows'>) {
  return scanAccountingFor(model.decision.items, model.summaryRows);
}
export function ScanTotals({ model }: { model: AppController }) {
  const data = scanAccounting(model);
  return (
    <div className="scan-accounting">
      <dl aria-label="扫描容量统计">
        <div>
          <dt>已扫描占用</dt>
          <dd>{scanAmount(data, 'observed')}</dd>
        </div>
        <div>
          <dt>可直接清理</dt>
          <dd>{scanAmount(data, 'available')}</dd>
        </div>
        <div>
          <dt>已保留</dt>
          <dd>{scanAmount(data, 'retained')}</dd>
        </div>
      </dl>
      <p>
        {data.pending
          ? '扫描尚未完成，以上仅为已读取范围。'
          : data.incomplete
            ? '部分位置未读取，以上仅为已读取范围。'
            : data.unsettled
              ? '结果需要重新核验，当前容量不作为清理依据。'
              : '仅统计本次已读取的文件范围。'}
        Windows 官方估算单独列出；勾选仅改变本次已选量。
      </p>
    </div>
  );
}
