import { selectableScan, ruleAssessment } from './scanAssessment';
import { isOfficialRule } from './ToolCache';
import type { AppController } from './useAppController';
import type { DecisionStatus, RuleDecision } from './cleanupDecision';

export type DeskStage =
  'idle' | 'scanning' | 'review' | 'cleaning' | 'complete' | 'attention' | 'stopping' | 'uncertain';
export type CleanupBucket = 'recommended' | 'review' | 'blocked';

export function isSelectableCleanup(item: RuleDecision) {
  return selectableScan(item);
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

export function ruleFollowup(item: RuleDecision) {
  if (item.scan?.pending) return { label: '等待扫描', next: '等待扫描完成；扫描不会删除文件。' };
  if (item.scan?.incomplete || item.scan?.state === 'stale')
    return {
      label: '需重新扫描',
      next: '查看读取问题，处理后重新扫描；重新扫描不会解除只读限制。',
    };
  if (item.scan && ['unscanned', 'not_scanned'].includes(item.scan.state))
    return { label: '尚未扫描', next: '开始扫描后查看本次占用；是否允许清理单独核验。' };
  return cleanupFollowup(item.status);
}
export function cleanupFollowupCounts(items: RuleDecision[]) {
  const counts = new Map<string, number>();
  for (const item of items) {
    const { label } = ruleFollowup(item);
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
  const incomplete = decision.items.filter((item) => {
    const scan = ruleAssessment(item, model.summaryRows);
    return !isOfficialRule(item.rule.id) && (scan.incomplete || scan.state === 'stale');
  });
  const found = decision.items.filter(
    (item) => (item.recognizedFiles ?? item.foundFiles) > 0 || incomplete.includes(item),
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
