import { assessScan, type ScanAssessment } from './scanAssessment';
import { cleanupEvidence } from './CleanupEvidence';
import { selectedTotals } from './selection';
import {
  applicationCapability,
  applicationCondition,
  applicationRuntimeBlock,
  type ApplicationCapability,
} from './applicationCapability';
import {
  cleanupResultSummary,
  priorityResultStatus,
  resultTargets,
  type ReviewContext,
} from './cleanupResults';
import type { ApplicationSupport, ExcludedFile, Report, Rule, RuleSummary, Session } from './types';

export type DecisionStatus =
  | 'ready'
  | 'running'
  | 'process_unknown'
  | 'readonly'
  | 'capability_unknown'
  | 'unsupported'
  | 'incomplete'
  | 'needs_scan'
  | 'empty'
  | 'absent'
  | 'excluded'
  | 'deselected'
  | 'unscanned'
  | 'blocked';
export type DecisionAction =
  | 'scan'
  | 'prepare'
  | 'refresh_apps'
  | 'verify'
  | 'stop'
  | 'review_unknown'
  | 'finish'
  | 'analysis'
  | 'choose';
export interface RuleDecision {
  rule: Rule;
  selected: boolean;
  status: DecisionStatus;
  label: string;
  explanation: string;
  recognizedFiles?: number;
  recognizedBytes?: bigint;
  foundFiles: number;
  foundBytes: bigint;
  files: number;
  bytes: bigint;
  incomplete: boolean;
  capability: ApplicationCapability;
  condition: string;
  scan?: ScanAssessment;
}
export const decisionLabels: Record<DecisionStatus, string> = {
  ready: '可清理',
  running: '需退出应用',
  process_unknown: '应用状态暂不可用',
  readonly: '尚未开放清理',
  capability_unknown: '能力未确认',
  unsupported: '当前不支持',
  incomplete: '检查未完成',
  needs_scan: '需要重新扫描',
  empty: '没有符合条件的文件',
  absent: '未发现标准位置',
  excluded: '已按设置排除',
  deselected: '文件已取消选择',
  unscanned: '等待扫描',
  blocked: '暂不可清理',
};

export function cleanupDecision(input: {
  rules: Rule[];
  applications?: ApplicationSupport[];
  rows: RuleSummary[];
  selected: Set<string>;
  excluded: Map<string, ExcludedFile>;
  included?: Map<string, ExcludedFile>;
  excludedRuleIds?: string[];
  sessions: Session[];
  reports: Report[];
  canPlan: boolean;
  working?: boolean;
  needsRescan: boolean;
  uncertain: boolean;
  active: 'scanning' | 'cleaning' | 'analysis' | null;
  stopping: boolean;
  reviewContext?: ReviewContext;
}) {
  const { rules, rows, selected, excluded, sessions, reports } = input;
  const applications = input.applications ?? [];
  const rowsById = new Map(rows.map((row) => [row.rule_id, row]));
  const applicationsById = new Map(applications.map((app) => [app.app_id, app]));
  const capabilities = new Map(rules.map((r) => [r.id, applicationCapability(r, applications)]));
  const runtimeBlocks = new Map(rules.map((r) => [r.id, applicationRuntimeBlock(r, applications)]));
  const assessments = new Map(
    rules.map((rule) => {
      const scope = rule.requires_admin ? 'system' : 'user';
      const job = input.reviewContext?.jobs[scope];
      return [
        rule.id,
        assessScan(rowsById.get(rule.id), {
          hasSession: sessions.length > 0,
          scanning: job ? job.state === 'scanning' : input.active === 'scanning',
          stale: input.needsRescan,
          excluded: input.excludedRuleIds?.includes(rule.id),
        }),
      ];
    }),
  );
  const eligibleRows = rows.map((row) => ({
    ...row,
    blocked:
      row.blocked ||
      !['completed', 'partial'].includes(assessments.get(row.rule_id)?.state ?? 'unscanned') ||
      !!input.excludedRuleIds?.includes(row.rule_id) ||
      !!row.availability ||
      !!runtimeBlocks.get(row.rule_id) ||
      capabilities.get(row.rule_id) !== 'allowed',
  }));
  const totals = selectedTotals(eligibleRows, selected, excluded, sessions, input.included);
  const items: RuleDecision[] = rules.map((rule) => {
    const row = rowsById.get(rule.id);
    const amount = totals.byRule.get(rule.id);
    const scan = assessments.get(rule.id)!;
    const incomplete = scan.incomplete;
    const capability = capabilities.get(rule.id)!;
    const support = rule.app_id ? applicationsById.get(rule.app_id) : undefined;
    let status: DecisionStatus = 'unscanned';
    let explanation = '只检查，确认后才删除。';
    if (input.excludedRuleIds?.includes(rule.id)) {
      status = 'excluded';
      explanation = '已按长期排除设置保留，不参与本次扫描与清理。';
    } else if (scan.absent) {
      status = 'absent';
      explanation = '未发现该标准位置，可能尚未安装或尚未生成文件。';
    } else if (capability === 'readonly') {
      status = 'readonly';
      explanation =
        cleanupEvidence(rule.id)?.conclusion ??
        (rule.root === 'developer' ? rule.impact : '此类内容尚未开放清理，已保留。');
    } else if (capability === 'unknown') {
      status = 'capability_unknown';
      explanation = '暂时无法确认这些内容能否清理，已保留。';
    } else if (row) {
      // Fresh application identity/layout restrictions also constrain an older
      // scan that only reported a running process. Exiting cannot unlock it.
      const code = runtimeBlocks.get(rule.id) ?? row.availability ?? row.scan_issue;
      if (code === 'validation_pending') {
        status = 'readonly';
        explanation = '此类内容尚未开放清理，已保留。';
      } else if (code === 'location_missing' && scan.absent) {
        status = 'absent';
        explanation = '未发现该标准位置，可能尚未安装或尚未生成文件；这与扫描后没有垃圾不同。';
      } else if (
        ['unsupported_version', 'application_unverified', 'unsupported_layout'].includes(code ?? '')
      ) {
        status = 'unsupported';
        explanation = '当前应用身份、资料位置或文件格式尚未通过核验，已保留。';
      } else if (code === 'browser_running' || code === 'application_running') {
        status = 'running';
        explanation = '应用正在运行，这些内容暂时保留。保存工作并退出后可重新检查。';
      } else if (code === 'process_unknown') {
        status = 'process_unknown';
        explanation = '暂时无法确认应用状态，请重新检查；不会强制结束应用。';
      } else if (['failed', 'interrupted'].includes(row.phase) || row.scan_failed) {
        status = 'incomplete';
        explanation = '本类别检查未完成，已读信息不构成清理授权。';
      } else if (['pending', 'scanning'].includes(row.phase)) {
        status = 'unscanned';
        explanation =
          row.phase === 'scanning' ? '正在检查，完成后可以选择。' : '此类别尚未完成检查。';
      } else if (
        input.needsRescan ||
        (!input.canPlan && !input.active && !input.uncertain && !reports.length && !code)
      ) {
        status = 'needs_scan';
        explanation = '当前扫描结果已失效，请重新扫描。';
      } else if (row.blocked || code) {
        status = 'blocked';
        explanation = row.note || '当前访问或验证条件不满足，请查看类别中的原因。';
      } else if (row.files === 0) {
        status = incomplete ? 'incomplete' : 'empty';
        explanation = incomplete
          ? '已读取部分未发现候选，仍有位置未检查。'
          : row.reasons.some((r) => r.code === 'recent' && r.count > 0)
            ? '近期修改的文件已保留，本次没有符合年龄条件的文件。'
            : row.reasons.some(
                  (r) => ['protected', 'excluded', 'link_or_cloud'].includes(r.code) && r.count > 0,
                )
              ? '本次发现的受保护内容已保留，没有可清理候选。'
              : '仅表示本次检查范围没有符合条件的文件。';
      } else if (selected.has(rule.id) && amount?.files === 0) {
        status = 'deselected';
        explanation = '本类别文件已全部取消选择，可在文件明细中重新选择。';
      } else {
        status = 'ready';
        explanation = incomplete ? '部分位置未读取，只能选择已通过检查的文件。' : rule.impact;
      }
    } else if (sessions.length) {
      status = 'needs_scan';
      explanation = '此类别不在当前扫描中，加入后需要重新扫描。';
    }
    return {
      rule,
      scan,
      selected: selected.has(rule.id) || (amount?.files ?? 0) > 0,
      status,
      label:
        status === 'readonly' && rule.root === 'developer'
          ? rule.id === 'dev.uv'
            ? '由 uv 整理闲置缓存'
            : '已保留，查看原因'
          : decisionLabels[status],
      explanation,
      recognizedFiles: (row?.files ?? 0) + (row?.observation?.retained_files ?? 0),
      recognizedBytes: BigInt(row?.bytes || '0') + BigInt(row?.observation?.retained_bytes || '0'),
      foundFiles: row?.files ?? 0,
      foundBytes: BigInt(row?.bytes ?? '0'),
      files: amount?.files ?? 0,
      bytes: amount?.bytes ?? 0n,
      incomplete,
      capability,
      condition: rule.app_id
        ? applicationCondition(support, row?.scan_issue ?? row?.availability, rule.id)
        : '',
    };
  });
  const chosen = items.filter((r) => r.selected);
  const result = cleanupResultSummary(reports);
  const reviewStatus = priorityResultStatus(reports);
  const targets = resultTargets(reports, rules, sessions, input.reviewContext);
  const has = (status: DecisionStatus) => chosen.some((r) => r.status === status);
  const canPrepare =
    input.canPlan &&
    !input.working &&
    !input.uncertain &&
    !input.active &&
    !input.stopping &&
    !input.needsRescan &&
    !reports.length &&
    totals.files > 0;
  let action: DecisionAction = sessions.length ? 'analysis' : 'scan';
  let message = '检查本版本支持的常见垃圾位置，扫描不会删除文件。';
  if (input.uncertain) {
    action = 'verify';
    message = '暂时无法确认任务状态，请只读核对；不会重复提交删除。';
  } else if (input.active || input.stopping) {
    action = 'stop';
    message = input.stopping
      ? '停止中，请等待后台确认。'
      : '任务进行中；停止不会撤销已经完成的删除。';
  } else if (reports.length) {
    action = result.state === 'unconfirmed' ? 'review_unknown' : 'finish';
    message =
      result.state === 'unconfirmed'
        ? '部分结果尚未确认，可查看记录；不会自动重试删除。'
        : result.stopped
          ? '清理已停止，已经删除的内容不会恢复。'
          : result.state === 'ended'
            ? '清理已结束，保留、失败和未处理原因可按需查看。'
            : '本次清理已完成。磁盘可用空间会自动刷新。';
  } else if (input.working && sessions.length) {
    action = 'prepare';
    message = '正在更新清理状态，请稍候…';
  } else if (canPrepare) {
    action = 'prepare';
    message = '已选内容可以清理，其他项目已保留。';
  } else if (!sessions.length) {
    action = 'scan';
  } else if (input.needsRescan) {
    action = 'scan';
    message = '请重新全面扫描，之后再选择本次清理文件。';
  } else if (!selected.size && !totals.files) {
    if (items.some((r) => ['ready', 'deselected'].includes(r.status))) {
      action = 'choose';
      message = '尚未选择清理文件。建议项可直接选择；需要你确认的项目请先查看影响。';
    } else if (items.some((r) => ['running', 'process_unknown'].includes(r.status))) {
      action = 'refresh_apps';
      message = items.some((r) => r.status === 'process_unknown')
        ? '部分运行状态无法确认，请重新检查；这不代表已确认缓存被占用。'
        : '相关应用仍在运行，请保存工作并正常退出后重新检查；不会自动结束应用。';
    } else if (items.some((item) => item.scan?.incomplete || item.scan?.state === 'stale')) {
      action = 'scan';
      message = '部分检查未完成，可查看原因后重新扫描；重新扫描不会开放只读项目。';
    } else {
      action = 'analysis';
      message = '本次没有可直接清理的已选文件。可查看各类别原因，或分析其他空间占用。';
    }
  } else if (input.needsRescan) {
    action = 'scan';
    message = '部分所选类别需要重新扫描；当前结果不能作为新增范围的计划。';
  } else if (has('running') || has('process_unknown')) {
    action = 'refresh_apps';
    message = '应用正在运行或状态无法确认，保存工作并退出后可以重新检查。';
  } else if (has('needs_scan')) {
    action = 'scan';
    message = '部分所选类别需要重新扫描；当前结果不能作为新增范围的计划。';
  } else if (has('readonly')) {
    message = '此类内容尚未开放清理，已保留。';
  } else if (has('capability_unknown')) {
    message = '部分内容暂时无法确认能否清理，已保留。';
  } else if (has('unsupported') || has('blocked')) {
    message = '所选内容暂不可清理，保留原因可展开查看。';
  } else if (has('incomplete')) {
    action = 'scan';
    message = '已读取部分未发现可清理文件，仍有位置未检查。';
  } else if (has('deselected')) {
    action = 'choose';
    message = '尚未选择可清理文件，可在明细中重新选择；无需为此重扫。';
  } else if (rows.length) {
    message = '所选范围内没有符合规则的文件，这不代表其他目录没有占用。';
  }
  const labels: Record<DecisionAction, string> = {
    scan: sessions.length ? '重新扫描' : '开始扫描',
    prepare: '清理已选',
    refresh_apps: '我已关闭，重新检查',
    verify: '只读核对任务状态',
    stop: input.stopping ? '停止中…' : '停止任务',
    review_unknown: '查看不确定项',
    finish: '完成',
    analysis: '查找空间大头',
    choose: '选择清理内容',
  };
  return {
    items,
    chosen,
    totals,
    result,
    reviewStatus,
    targets,
    canPrepare,
    action,
    actionLabel: labels[action],
    message,
    phase: input.uncertain
      ? 'uncertain'
      : (input.active ?? (reports.length ? 'result' : rows.length ? 'review' : 'idle')),
  };
}
export type CleanupDecision = ReturnType<typeof cleanupDecision>;
