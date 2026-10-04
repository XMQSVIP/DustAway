import type {
  Candidate,
  Job,
  ReasonCode,
  Report,
  Rule,
  RuleOutcome,
  Scope,
  Session,
  Summary,
} from './types';
import { formatBytes } from './i18n';

export type ResultStatus = 'deleted' | 'skipped' | 'failed' | 'unknown' | 'candidate';
export const resultLabels: Record<ResultStatus, string> = {
  deleted: '已删除',
  skipped: '跳过并保留',
  failed: '失败',
  unknown: '状态不确定',
  candidate: '未处理',
};
type Counts = Pick<RuleOutcome, 'total' | 'deleted' | 'skipped' | 'failed' | 'unknown'>;
const countKeys = ['total', 'deleted', 'skipped', 'failed', 'unknown'] as const;
export function unprocessed(result: Counts) {
  if (!validCounts(result)) return Number.NaN;
  return Math.max(
    0,
    result.total - result.deleted - result.skipped - result.failed - result.unknown,
  );
}
export const resultCount = (result: Counts, status: ResultStatus) =>
  status === 'candidate' ? unprocessed(result) : result[status];

export function validCountNumbers(result: Counts) {
  return countKeys.every((key) => Number.isSafeInteger(result[key]) && result[key] >= 0);
}
export const formatResultCount = (value: number) =>
  Number.isSafeInteger(value) && value >= 0 ? value.toLocaleString() : '统计不可用';
export function recordedBytes(value: string, signed = false): bigint | null {
  if (typeof value !== 'string' || !(signed ? /^-?\d{1,20}$/ : /^\d{1,20}$/).test(value))
    return null;
  const bytes = BigInt(value),
    limit = 18_446_744_073_709_551_615n;
  return bytes > limit || bytes < (signed ? -limit : 0n) ? null : bytes;
}
export function formatRecordedBytes(value: string, signed = false) {
  const bytes = recordedBytes(value, signed);
  return bytes === null ? '统计不可用' : formatBytes(bytes);
}
export function formatRecordedTotal(values: string[]) {
  const parsed = values.map((value) => recordedBytes(value));
  return parsed.some((value) => value === null)
    ? '统计不可用'
    : formatBytes(parsed.reduce<bigint>((n, v) => n + v!, 0n));
}
function validCounts(result: Counts) {
  return (
    validCountNumbers(result) &&
    result.deleted + result.skipped + result.failed + result.unknown <= result.total
  );
}
export function consistentReport(report: Report) {
  if (
    recordedBytes(report.deleted_bytes) === null ||
    recordedBytes(report.candidate_bytes) === null
  )
    return false;
  if (!validCounts(report)) return false;
  if (report.directories && !validCounts(report.directories)) return false;
  // Old successful reports may have no category breakdown; they cannot establish normal retention.
  if (!report.categories?.length && !report.manual) return true;
  return (
    new Set((report.categories ?? []).map((c) => c.rule_id)).size ===
      (report.categories ?? []).length &&
    (report.categories ?? []).every(
      (c) => validCounts(c) && recordedBytes(c.deleted_bytes) !== null,
    ) &&
    (!report.manual ||
      (validCounts(report.manual) && recordedBytes(report.manual.deleted_bytes) !== null)) &&
    countKeys.every(
      (key) =>
        (report.categories ?? []).reduce((n, c) => n + c[key], report.manual?.[key] ?? 0) ===
        report[key],
    )
  );
}
const retentionReasons = new Set(['recent', 'protected', 'excluded', 'link_or_cloud']);
export function expectedRetention(report: Report) {
  const outcomes = [...(report.categories ?? []), ...(report.manual ? [report.manual] : [])];
  return (
    report.status === 'finished' &&
    report.skipped > 0 &&
    !report.failed &&
    !report.unknown &&
    !unprocessed(report) &&
    consistentReport(report) &&
    !!outcomes.length &&
    outcomes.every((category) => {
      const reasons = category.reasons ?? [];
      return (
        new Set(reasons.map((r) => r.code)).size === reasons.length &&
        reasons.every(
          (r) => retentionReasons.has(r.code) && Number.isSafeInteger(r.count) && r.count > 0,
        ) &&
        reasons.reduce((n, r) => n + r.count, 0) === category.skipped
      );
    })
  );
}
export function priorityResultStatus(reports: Report[]): ResultStatus | null {
  if (reports.some((r) => !consistentReport(r))) return 'unknown';
  for (const status of ['unknown', 'failed', 'candidate'] as const)
    if (
      reports.some(
        (r) =>
          resultCount(r, status) > 0 || (r.directories && resultCount(r.directories, status) > 0),
      )
    )
      return status;
  return reports.some(
    (r) => (r.skipped > 0 && !expectedRetention(r)) || (r.directories?.skipped ?? 0) > 0,
  )
    ? 'skipped'
    : null;
}
export function fullyDeleted(report: Report) {
  return (
    report.status === 'finished' &&
    report.total + (report.directories?.total ?? 0) > 0 &&
    report.deleted === report.total &&
    !report.skipped &&
    !report.failed &&
    !report.unknown &&
    (!report.directories || report.directories.deleted === report.directories.total) &&
    consistentReport(report)
  );
}
export function resultTitle(report: Report) {
  return cleanupResultSummary([report]).title;
}

/** One display model for the current receipt and historical reports. Never repairs stored data. */
export function cleanupResultSummary(reports: Report[]) {
  const sum = (values: number[]) => {
    const value = values.reduce((n, v) => n + v, 0);
    return values.every((v) => Number.isSafeInteger(v) && v >= 0) && Number.isSafeInteger(value)
      ? value
      : Number.NaN;
  };
  const counts = (items: Counts[]) => ({
    ...(Object.fromEntries(countKeys.map((key) => [key, sum(items.map((r) => r[key]))])) as Record<
      (typeof countKeys)[number],
      number
    >),
    unprocessed: sum(items.map(unprocessed)),
  });
  const files = counts(reports),
    directories = counts(reports.flatMap((r) => (r.directories ? [r.directories] : [])));
  const priority = priorityResultStatus(reports);
  const stopped = reports.some((r) => r.status === 'cancelled' || r.status === 'interrupted');
  const state =
    priority === 'unknown'
      ? 'unconfirmed'
      : stopped || priority || reports.some((r) => r.status !== 'finished')
        ? 'ended'
        : 'complete';
  const byteValues = reports.map((r) => recordedBytes(r.deleted_bytes));
  return {
    files,
    directories,
    state,
    title:
      state === 'unconfirmed' ? '部分结果尚未确认' : state === 'ended' ? '清理已结束' : '清理完成',
    stopped,
    total: files.total,
    deleted: files.deleted,
    skipped: files.skipped,
    failed: sum([files.failed, directories.failed]),
    unknown: sum([files.unknown, directories.unknown]),
    unprocessed: sum([files.unprocessed, directories.unprocessed]),
    expectedRetention:
      reports.some((r) => r.skipped > 0) &&
      reports.every((r) => !r.skipped || expectedRetention(r)),
    bytes: byteValues.every((v) => v !== null) ? byteValues.reduce((n, v) => n + v!, 0n) : null,
    priority,
  };
}

export interface ResultTarget {
  key: string;
  reportId: string;
  scope: Scope;
  session: string | null;
  ruleId: string;
  title: string;
  status: ResultStatus;
  count: number;
}
export interface ReviewContext {
  jobs: Partial<Record<Scope, Job>>;
  summaries: Partial<Record<Scope, Summary>>;
}
export function resultTargets(
  reports: Report[],
  rules: Rule[],
  sessions: Session[],
  context?: ReviewContext,
) {
  return reports
    .flatMap((report) =>
      (report.categories ?? []).flatMap((category) => {
        const rule = rules.find((r) => r.id === category.rule_id);
        const session = sessions.find((s) => s.scope === report.scope)?.id;
        const job = context?.jobs[report.scope];
        const summary = context?.summaries[report.scope];
        const available =
          !!rule &&
          rule.requires_admin === (report.scope === 'system') &&
          !!session &&
          session === job?.id &&
          session === summary?.session_id &&
          summary.state === job?.state &&
          ['finished', 'cancelled', 'interrupted'].includes(job?.state ?? '') &&
          summary.rules.some((r) => r.rule_id === category.rule_id);
        return (['unknown', 'failed', 'candidate', 'skipped', 'deleted'] as const).flatMap(
          (status) => {
            const count = resultCount(category, status);
            return Number.isSafeInteger(count) && count > 0
              ? [
                  {
                    key: `${report.scope}:${report.id}:${session ?? ''}:${category.rule_id}:${status}`,
                    reportId: report.id,
                    scope: report.scope,
                    session: available ? session! : null,
                    ruleId: category.rule_id,
                    title: rule?.title ?? '历史清理类别',
                    status,
                    count,
                  },
                ]
              : [];
          },
        );
      }),
    )
    .sort(
      (a, b) => b.count - a.count || a.ruleId.localeCompare(b.ruleId) || a.key.localeCompare(b.key),
    );
}

export function reasonNextStep(code?: ReasonCode | null) {
  switch (code) {
    case 'in_use':
    case 'browser_running':
    case 'application_running':
      return '保存工作并正常退出相关应用后重新扫描，再选择需要清理的文件。';
    case 'changed':
      return '重新扫描以核对最新文件，再决定是否清理。';
    case 'recent':
      return '等满足保留期限后重新扫描；符合规则才可选择。';
    case 'protected':
    case 'link_or_cloud':
      return '当前不提供直接清理，请继续保留。';
    case 'excluded':
      return '如需纳入清理，先核对排除设置，再重新扫描。';
    case 'validation_pending':
    case 'unsupported_version':
    case 'unsupported_layout':
    case 'unsupported_volume':
      return '当前条件尚不支持清理，重复扫描不会自动解除限制。';
    case 'process_unknown':
      return '重新检查应用状态；确认通过后才可选择，不会强制结束进程。';
    case 'access_denied':
    case 'directory_inaccessible':
      return '检查该位置是否可访问，处理后重新扫描；不会自动修改权限。';
    case 'unknown':
      return '先核对原始记录和文件现状，不会自动重试删除。';
    default:
      return '查看具体原因，处理后重新扫描；重新检查不保证能够清理。';
  }
}

export function candidateNextStep(file: Candidate) {
  if (file.status === 'deleted') return '';
  if (file.status === 'unknown') return '删除结果尚未确认，不会自动重试。';
  if (file.status === 'candidate') return '本次未执行删除。如需继续，请重新扫描并选择。';
  if (file.status === 'failed')
    return `未能完成删除，原因可展开查看。${reasonNextStep(file.reason_code)}`;
  switch (file.reason_code) {
    case 'in_use':
    case 'browser_running':
    case 'application_running':
      return `文件正在使用，本次未删除。${reasonNextStep(file.reason_code)}`;
    case 'changed':
      return `文件或所在目录已变化，本次未删除。${reasonNextStep(file.reason_code)}`;
    case 'recent':
      return `最近仍有修改，本次未删除。${reasonNextStep(file.reason_code)}`;
    case 'protected':
      return `受保护内容，本次未删除。${reasonNextStep(file.reason_code)}`;
    case 'excluded':
      return `已按长期排除设置跳过，本次未删除。${reasonNextStep(file.reason_code)}`;
    case 'link_or_cloud':
      return `链接或云文件，本次未删除。${reasonNextStep(file.reason_code)}`;
    case 'access_denied':
      return `当前权限无法处理，本次未删除。${reasonNextStep(file.reason_code)}`;
    default:
      return `本次未删除，原因可展开查看。${reasonNextStep(file.reason_code)}`;
  }
}
