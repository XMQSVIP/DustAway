import type { Job, RuleSummary, Scope, Session, Summary } from './types';
import type { RuleDecision } from './cleanupDecision';
import { formatBytes, reasonLabels } from './i18n';

// Display assessment only. Neither this model nor its byte totals grants deletion authority.
export const isOfficialRule = (id: string) =>
  ['dev.uv', 'dev.go_build', 'dev.yarn', 'dev.ccache'].includes(id);
export type ScanState =
  | 'unscanned'
  | 'not_scanned'
  | 'excluded'
  | 'pending'
  | 'scanning'
  | 'unvisited'
  | 'completed'
  | 'partial'
  | 'interrupted'
  | 'failed'
  | 'stale';
const labels: Record<ScanState, string> = {
  unscanned: '尚未扫描',
  not_scanned: '本次未扫描',
  excluded: '已按设置排除',
  pending: '等待扫描',
  scanning: '扫描中',
  unvisited: '尚未检查',
  completed: '扫描完成',
  partial: '部分检查完成',
  interrupted: '扫描已中断',
  failed: '读取失败',
  stale: '结果已失效',
};
export function recordedBytes(value: string | null | undefined): bigint | null {
  return typeof value === 'string' && /^\d+$/.test(value) ? BigInt(value) : null;
}
export function assessScan(
  row: RuleSummary | undefined,
  context: { hasSession?: boolean; scanning?: boolean; stale?: boolean; excluded?: boolean } = {},
) {
  let state: ScanState = !row ? (context.hasSession ? 'not_scanned' : 'unscanned') : 'completed';
  if (context.excluded) state = 'excluded';
  else if (row) {
    if (context.stale || row.stale) state = 'stale';
    else if (row.phase === 'failed' || row.scan_failed) state = 'failed';
    else if (row.phase === 'interrupted') state = 'interrupted';
    else if (row.phase === 'pending') state = context.scanning ? 'pending' : 'unvisited';
    else if (row.phase === 'scanning') state = context.scanning ? 'scanning' : 'interrupted';
    else if (
      row.phase === 'partial' ||
      row.coverage?.complete === false ||
      row.coverage?.issues.length
    )
      state = 'partial';
  }
  const participating = !!row && state !== 'excluded';
  const read = participating && !['pending', 'unvisited'].includes(state);
  const candidate = read ? recordedBytes(row.bytes) : 0n;
  const retainedBase = read ? recordedBytes(row.observation?.retained_bytes) : 0n;
  const files = read ? row.files + (row.observation?.retained_files ?? 0) : 0;
  const observed = (candidate ?? 0n) + (retainedBase ?? 0n);
  const incomplete = ['partial', 'failed', 'interrupted', 'unvisited'].includes(state);
  const pending = ['pending', 'scanning'].includes(state);
  const unsettled = pending || state === 'stale';
  const absent =
    state === 'completed' &&
    row?.coverage?.complete === true &&
    row.scan_issue === 'location_missing' &&
    files === 0 &&
    observed === 0n;
  const unknown =
    participating &&
    (incomplete ||
      unsettled ||
      !row.coverage ||
      !row.observation ||
      candidate === null ||
      retainedBase === null ||
      row.skipped > row.observation.retained_files);
  return {
    state,
    label: absent ? '未发现标准位置' : labels[state],
    participating,
    read,
    candidate: candidate ?? 0n,
    retainedBase: retainedBase ?? 0n,
    observed,
    files,
    incomplete,
    pending,
    unsettled,
    absent,
    unknown,
  };
}
export type ScanAssessment = ReturnType<typeof assessScan>;

/** A mismatched response never contributes bytes to a newer user/system session. */
export function currentScanRows(
  sessions: Session[],
  summaries: Partial<Record<Scope, Summary>>,
  jobs: Partial<Record<Scope, Job>>,
  stale = false,
): RuleSummary[] {
  return sessions.flatMap(({ scope, id }) => {
    const summary = summaries[scope],
      job = jobs[scope];
    if (
      !summary ||
      !job ||
      summary.session_id !== id ||
      job.id !== id ||
      summary.state !== job.state
    )
      return [];
    return summary.rules.map((row) => ({
      ...row,
      blocked: row.blocked || job.state !== 'ready' || stale,
    }));
  });
}
export function ruleAssessment(item: RuleDecision, rows: RuleSummary[]) {
  return (
    item.scan ??
    assessScan(
      rows.find((row) => row.rule_id === item.rule.id),
      {
        hasSession: rows.length > 0,
        excluded: item.status === 'excluded',
      },
    )
  );
}
export function selectableScan(item: RuleDecision) {
  return (
    item.capability === 'allowed' &&
    ['ready', 'deselected'].includes(item.status) &&
    (!item.scan || ['completed', 'partial'].includes(item.scan.state))
  );
}
export interface RetentionReason {
  label: string;
  bytes: bigint | null;
  count: number;
}
export function scanAccountingFor(items: RuleDecision[], rows: RuleSummary[]) {
  const assessed = items
    .filter((item) => !isOfficialRule(item.rule.id))
    .map((item) => ({
      item,
      scan: ruleAssessment(item, rows),
      row: rows.find((r) => r.rule_id === item.rule.id),
    }))
    .filter(({ scan }) => scan.participating);
  let observed = 0n,
    available = 0n,
    retained = 0n;
  const reasons = new Map<string, RetentionReason>();
  const add = (label: string, bytes: bigint | null, count: number) => {
    // Preserve the known contribution when the same cause also has unmeasured entries.
    const key = `${label}:${bytes === null ? 'unknown' : 'known'}`;
    const old = reasons.get(key);
    reasons.set(key, {
      label,
      count: (old?.count ?? 0) + count,
      bytes: bytes === null || old?.bytes === null ? null : (old?.bytes ?? 0n) + bytes,
    });
  };
  for (const { item, scan, row } of assessed) {
    observed += scan.observed;
    const canChoose = selectableScan(item) && !scan.unsettled;
    available += canChoose ? scan.candidate : 0n;
    retained += scan.retainedBase + (!canChoose && !scan.unsettled ? scan.candidate : 0n);
    if (!row || !scan.read) continue;
    const quantified = new Set<string>();
    for (const reason of row.observation?.reasons ?? []) {
      if (reason.files > 0) {
        add(reasonLabels[reason.code], recordedBytes(reason.bytes), reason.files);
        quantified.add(reason.code);
      }
    }
    for (const reason of row.reasons ?? []) {
      if (reason.count > 0 && !quantified.has(reason.code))
        add(reasonLabels[reason.code], null, reason.count);
    }
    if (!canChoose && !scan.unsettled && row.files > 0)
      add(item.capability === 'readonly' ? '尚未开放清理' : item.label, scan.candidate, row.files);
  }
  return {
    observed,
    available,
    retained,
    participating: assessed.length > 0,
    read: assessed.some(({ scan }) => scan.read),
    incomplete: assessed.some(({ scan }) => scan.incomplete),
    unknown: assessed.some(({ scan }) => scan.unknown),
    pending: assessed.some(({ scan }) => scan.pending),
    unsettled: assessed.some(({ scan }) => scan.unsettled),
    absent: assessed.length > 0 && assessed.every(({ scan }) => scan.absent),
    reasons: [...reasons.values()].sort((a, b) =>
      a.bytes === b.bytes
        ? a.label.localeCompare(b.label)
        : a.bytes === null
          ? 1
          : b.bytes === null
            ? -1
            : a.bytes > b.bytes
              ? -1
              : 1,
    ),
  };
}
export type ScanAccounting = ReturnType<typeof scanAccountingFor>;
export function scanAmount(data: ScanAccounting, kind: 'observed' | 'available' | 'retained') {
  if (!data.participating) return '待扫描';
  if (data.absent) return '未发现标准位置';
  if (kind !== 'observed' && data.unsettled) return '待确定';
  if (!data.read) return data.pending ? '待扫描' : '尚未检查';
  if (kind === 'available')
    return data.unknown && data.available === 0n ? '待核验' : formatBytes(data.available);
  return data.unknown
    ? data[kind] === 0n
      ? '未知大小'
      : `${formatBytes(data[kind])} + 未知大小`
    : formatBytes(data[kind]);
}
export function retainedReasonText(reasons: RetentionReason[], limit = 2) {
  return reasons
    .slice(0, limit)
    .map((r) => r.label + (r.bytes === null ? '（容量未知）' : ''))
    .join('、');
}
