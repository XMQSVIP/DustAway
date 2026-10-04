import { isOfficialRule } from './ToolCache';
import type { AppController } from './useAppController';
import type { DecisionStatus, RuleDecision } from './cleanupDecision';

export type DeskStage =
  'idle' | 'scanning' | 'review' | 'cleaning' | 'complete' | 'attention' | 'stopping' | 'uncertain';
export type CleanupBucket = 'recommended' | 'review' | 'blocked';

export function isSelectableCleanup(item: RuleDecision) {
  return item.capability === 'allowed' && ['ready', 'deselected'].includes(item.status);
}

export function cleanupFollowup(status: DecisionStatus) {
  switch (status) {
    case 'running':
      return {
        label: '需退出应用',
        next: '保存工作并正常退出相关应用及后台组件，再重新检查。通过核验后才可选择清理。',
      };
    case 'process_unknown':
      return {
        label: '需确认运行状态',
        next: '重新检查应用状态；确认通过后才可选择，不会强制结束进程。',
      };
    case 'incomplete':
    case 'needs_scan':
    case 'capability_unknown':
    case 'blocked':
      return {
        label: '需重新检查',
        next: '查看具体原因，处理后重新扫描；重新检查不保证能够清理。',
      };
    case 'readonly':
      return {
        label: '尚未开放清理',
        next: '当前仅支持查看；需完成对应清理能力的验证后才能开放，重复扫描不会解除此限制。',
      };
    case 'unsupported':
      return {
        label: '当前不支持清理',
        next: '当前应用身份、目录或格式未通过核验，暂不能在此清理。',
      };
    case 'excluded':
      return { label: '已按设置排除', next: '如需纳入清理，请先核对排除设置，再重新扫描。' };
    case 'empty':
      return {
        label: '没有符合条件的文件',
        next: '查看保留期限和保护规则；本次没有可选择的文件。',
      };
    case 'absent':
      return {
        label: '未发现位置',
        next: '当前没有发现支持的资料位置，不能据此判断整个应用没有占用。',
      };
    case 'unscanned':
      return { label: '等待检查', next: '等待扫描完成后查看结果。' };
    case 'ready':
    case 'deselected':
      return { label: '可自行选择', next: '查看文件及清理影响，勾选后才会纳入确认清单。' };
  }
}

export function cleanupFollowupCounts(items: RuleDecision[]) {
  const counts = new Map<string, number>();
  for (const item of items) {
    const { label } = cleanupFollowup(item.status);
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }
  return [...counts].map(([label, count]) => `${label} ${count} 类`).join(' · ');
}

/** Presentation only. Selection and deletion authority stay in the controller/backend. */
export function cleanupPresentation(model: AppController) {
  const { decision, currentJob, reports } = model;
  const terminal = reports.length > 0 && !model.hasManual;
  const stage: DeskStage = model.uncertain
    ? 'uncertain'
    : model.stopping
      ? 'stopping'
      : currentJob?.state === 'scanning'
        ? 'scanning'
        : currentJob?.state === 'cleaning'
          ? 'cleaning'
          : terminal
            ? decision.result.state !== 'complete'
              ? 'attention'
              : 'complete'
            : model.scanned || model.hasManual
              ? 'review'
              : 'idle';
  const found = decision.items.filter(
    (item) =>
      (item.recognizedFiles ?? item.foundFiles) > 0 ||
      model.summaryRows.some(
        (row) =>
          row.rule_id === item.rule.id &&
          row.scan_issue !== 'location_missing' &&
          (row.scan_failed || (row.coverage?.issues.length ?? 0) > 0),
      ),
  );
  const incomplete = decision.items.filter((item) =>
    model.summaryRows.some(
      (row) =>
        row.rule_id === item.rule.id &&
        row.scan_issue !== 'location_missing' &&
        (row.scan_failed ||
          ['partial', 'failed', 'interrupted'].includes(row.phase) ||
          row.coverage?.complete === false),
    ),
  );
  const buckets: { key: CleanupBucket; title: string; hint: string; items: RuleDecision[] }[] = [
    {
      key: 'recommended',
      title: '建议清理',
      hint: '常规垃圾，可按需选择',
      items: found.filter((r) => isSelectableCleanup(r) && r.rule.cleanup_policy === 'recommended'),
    },
    {
      key: 'review',
      title: '可自行选择',
      hint: '先看影响，再决定是否清理',
      items: found.filter((r) => isSelectableCleanup(r) && r.rule.cleanup_policy !== 'recommended'),
    },
    {
      key: 'blocked',
      title: '本次未清理',
      hint: '查看原因，其他项目可先处理',
      items: found.filter((r) => !isSelectableCleanup(r)),
    },
  ];
  const available = found.filter(isSelectableCleanup);
  // Official maintenance is actionable, but never a per-file deletion selection.
  const official = found.filter((r) => isOfficialRule(r.rule.id));
  buckets[2].items = buckets[2].items.filter((r) => !isOfficialRule(r.rule.id));
  const running = decision.items.filter(
    (r) => r.capability === 'allowed' && r.status === 'running',
  );
  const processUnknown = decision.items.filter(
    (r) => r.capability === 'allowed' && r.status === 'process_unknown',
  );
  return {
    stage,
    terminal,
    found,
    incomplete,
    buckets,
    available,
    official,
    running,
    processUnknown,
  };
}
