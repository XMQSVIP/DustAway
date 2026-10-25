import type { AppController } from './useAppController';
import { scanAccounting } from './ScanTotals';
import { retainedReasonText } from './scanAssessment';

/** Derive all three dimensions from the same snapshot; selection never changes occupancy. */
export function weixinStatus(model: AppController) {
  const items = model.decision.items.filter((item) => item.rule.id.startsWith('weixin.'));
  const rows = model.summaryRows.filter((row) => row.rule_id.startsWith('weixin.'));
  const totals = scanAccounting({ decision: { ...model.decision, items }, summaryRows: rows });
  const scanned = totals.read;
  const pending = totals.pending;
  const incomplete = totals.incomplete;
  const unknown = totals.unknown;
  const support = model.applicationInfo?.applications.find((app) => app.app_id === 'weixin');
  const runtime = model.applicationInfo?.loading
    ? '正在核对'
    : model.applicationInfo?.error ||
        !support?.identity_verified ||
        support.status === 'process_unknown' ||
        support.rules?.some((rule) => rule.cleanup_block === 'process_unknown')
      ? '尚未确认'
      : support.running_components.length
        ? '微信或后台组件正在运行'
        : '未检测到微信进程';
  const allowed = items.filter((item) => item.capability === 'allowed').length;
  const permission = allowed
    ? `${allowed} 项已开放，仍需核验当前条件`
    : items.some((item) => item.capability === 'unknown')
      ? '部分清理能力尚未确认'
      : '当前缓存与日志仅供查看';
  const progress =
    totals.unsettled && !pending
      ? '结果已失效'
      : pending
        ? '扫描中'
        : incomplete
          ? '扫描未完整结束'
          : scanned
            ? '扫描完成'
            : '待扫描';
  return {
    items,
    totals,
    scanned,
    incomplete,
    unknown,
    progress,
    permission,
    runtime,
    reasons: totals.reasons.map((reason) => retainedReasonText([reason])),
  };
}
