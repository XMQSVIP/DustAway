import { ruleAssessment, scanAccountingFor, scanAmount, recordedBytes } from './scanAssessment';
import { CleanupEvidence, cleanupEvidence } from './CleanupEvidence';
import { formatRecordedBytes, formatResultCount } from './cleanupResults';
import { applicationScope } from './applicationCapability';
import { ChevronRight, FileLock2 } from 'lucide-react';
import { CoverageNotice } from './CoverageNotice';
import { dynamic, formatBytes, scanPhases, ui } from './i18n';
import { ReasonList } from './ReasonList';
import { ResultCounts } from './ResultCounts';
import { ApplicationSupportNotice } from './ApplicationSupport';
import { useState } from 'react';
import { ScanObservations } from './ScanObservations';
import type { Rule } from './types';
import type { AppController } from './useAppController';
import { ruleFollowup, isSelectableCleanup } from './cleanupPresentation';

export function CleanupRuleList({
  model,
  rules,
  catalog = false,
}: {
  model: AppController;
  rules: Rule[];
  catalog?: boolean;
}) {
  const [evidenceOpen, setEvidenceOpen] = useState(new Set<string>());
  const [observationsOpen, setObservationsOpen] = useState(new Set<string>());
  return (
    <div className="rule-list">
      {rules.map((rule) => {
        const row = model.summaryRows.find((s) => s.rule_id === rule.id);
        const item = model.decision.items.find((r) => r.rule.id === rule.id);
        const scan = item ? ruleAssessment(item, model.summaryRows) : undefined;
        const accounting = item ? scanAccountingFor([item], model.summaryRows) : undefined;
        const outcome = model.reports
          .flatMap((r) => r.categories ?? [])
          .find((r) => r.rule_id === rule.id);
        const terminal = model.reports.length > 0;
        const amount = model.totals.byRule.get(rule.id);
        const selected = !!amount?.files && amount.files === row?.files;
        const partial = !!amount?.files && amount.files < (row?.files ?? 0);
        const editable =
          !catalog &&
          !terminal &&
          !!item &&
          isSelectableCleanup(item) &&
          item?.capability === 'allowed';
        const canDetail =
          !!row?.files &&
          (!model.busy ||
            (model.currentJob?.state === 'scanning' &&
              ['completed', 'partial'].includes(row.phase)));
        const outsidePlan = Math.max(0, (row?.files ?? 0) - (outcome?.total ?? 0));
        const measured = !!scan?.read && (scan.files > 0 || !scan.unknown);
        return (
          <article className="rule-row" key={rule.id} data-rule={rule.id}>
            {editable ? (
              <input
                type="checkbox"
                id={`choose-${rule.id}`}
                aria-label={rule.title}
                checked={selected}
                aria-checked={partial ? 'mixed' : selected}
                ref={(element) => {
                  if (element) element.indeterminate = partial;
                }}
                disabled={model.busy || model.saving}
                onChange={(e) => model.toggle(rule.id, e.target.checked)}
              />
            ) : (
              <FileLock2 size={18} className="rule-lock" aria-hidden="true" />
            )}
            <div className="rule-copy">
              <div className="rule-title">
                <label htmlFor={editable ? `choose-${rule.id}` : undefined}>{rule.title}</label>
                {rule.cleanup_policy !== 'recommended' && item?.capability === 'allowed' && (
                  <span className="review-label">自行选择</span>
                )}
                {item?.capability === 'readonly' && (
                  <span className="readonly-label">尚未开放清理</span>
                )}
              </div>
              {!catalog && !terminal && scan && (scan.state !== 'completed' || scan.absent) && (
                <p className="rule-scan-status" data-scan-state={scan.state}>
                  {scan.label}
                  {scan.incomplete ? ' · 已读容量不代表完整占用' : ''}
                </p>
              )}
              <p>
                {catalog
                  ? rule.description
                  : editable
                    ? rule.impact
                    : (item?.explanation ?? rule.description)}
              </p>
              {!catalog && !terminal && item && !isSelectableCleanup(item) && (
                <p className="muted">后续处理：{ruleFollowup(item).next}</p>
              )}
              {terminal && row && scan && (
                <small>
                  扫描时占用 {accounting ? scanAmount(accounting, 'observed') : '未知大小'} · 已读{' '}
                  {scan.files} 个文件
                </small>
              )}
              {terminal && outcome && (
                <ResultCounts
                  result={outcome}
                  onDetail={(status) => model.openDetail(rule, 'cleanup', status)}
                />
              )}
              {terminal && outsidePlan > 0 && (
                <button
                  className="result-link"
                  disabled={model.busy}
                  onClick={() => model.openDetail(rule, 'unplanned')}
                >
                  查看未纳入计划的 {outsidePlan} 个文件
                </button>
              )}
              <CleanupEvidence ruleId={rule.id} />
              {row?.observation?.application && (
                <p className="muted">
                  {row.observation.application.identity_status === 'verified'
                    ? '安装已核验'
                    : row.observation.application.identity_status === 'registered'
                      ? '检测到安装登记；不代表已取得清理许可'
                      : '发现相关数据，安装身份未确认'}
                  {!row.observation.application.inventory_complete && '；安装信息读取不完整'}
                </p>
              )}
              {!!row?.observation?.retained_files && (
                <p className="muted">
                  {terminal ? '扫描时保留内容' : '已读保留内容'}{' '}
                  {recordedBytes(row.observation.retained_bytes) === null
                    ? '未知大小'
                    : formatBytes(row.observation.retained_bytes)}{' '}
                  · {row.observation.retained_files} 个文件，不计入可清理大小。
                  {!!row.observation.recent_files &&
                    `其中 ${row.observation.recent_files} 个文件未满足最后修改时间条件。`}
                </p>
              )}
              {!!row?.skipped &&
                model.sessions.some(
                  (s) => s.scope === (rule.requires_admin ? 'system' : 'user'),
                ) && (
                  <details
                    onToggle={(event) => {
                      const open = event.currentTarget.open;
                      setObservationsOpen((previous) => {
                        const next = new Set(previous);
                        open ? next.add(rule.id) : next.delete(rule.id);
                        return next;
                      });
                    }}
                  >
                    <summary>查看已保留文件</summary>
                    {observationsOpen.has(rule.id) && (
                      <ScanObservations
                        key={`${model.sessionKey}:${rule.id}`}
                        scope={rule.requires_admin ? 'system' : 'user'}
                        session={
                          model.sessions.find(
                            (s) => s.scope === (rule.requires_admin ? 'system' : 'user'),
                          )!.id
                        }
                        rule={rule.id}
                      />
                    )}
                  </details>
                )}
              <details
                className="rule-evidence"
                onToggle={(event) => {
                  const open = event.currentTarget.open;
                  setEvidenceOpen((previous) => {
                    const next = new Set(previous);
                    open ? next.add(rule.id) : next.delete(rule.id);
                    return next;
                  });
                }}
              >
                <summary>{catalog ? '处理影响与检查说明' : '技术详情'}</summary>
                <p>{rule.impact}</p>
                {rule.min_age_days > 0 && (
                  <p>
                    仅列出最后修改时间距今满{' '}
                    {rule.min_age_days === 1 ? '24 小时（86,400 秒）' : `${rule.min_age_days} 天`}
                    的文件；近期文件保留。
                  </p>
                )}
                {item?.condition && <p>{item.condition}</p>}
                {!!row?.observation?.application?.locations.length && (
                  <p className="cache-locations">
                    识别的数据位置：{row.observation.application.locations.join('；')}
                  </p>
                )}
                {row && <CoverageNotice coverage={row.coverage} />}
                {item?.capability === 'readonly' &&
                  !cleanupEvidence(rule.id) &&
                  rule.root !== 'developer' && <p>尚未完成真实应用删除验收，仅支持扫描与查看。</p>}
                {rule.app_id && model.applicationInfo && evidenceOpen.has(rule.id) && (
                  <ApplicationSupportNotice
                    rules={model.boot?.rules ?? []}
                    support={model.applicationInfo.applications.find(
                      (app) => app.app_id === rule.app_id,
                    )}
                    loading={model.applicationInfo.loading}
                    error={model.applicationInfo.error}
                    busy={model.busy}
                    onRefresh={model.refreshAvailability}
                  />
                )}
                {applicationScope(rule) && <p>{applicationScope(rule)}</p>}
                {model.boot?.settings.excluded_rules.includes(rule.id) && <p>{ui.excludedRule}</p>}
                {row ? (
                  <>
                    <p>
                      {scan?.label ?? scanPhases[row.phase]} · {item?.label}{' '}
                      {item?.capability !== 'readonly' && row.note}
                    </p>
                    {row.files === 0 &&
                      row.phase === 'completed' &&
                      row.coverage?.complete !== false &&
                      !row.scan_issue && (
                        <p>本次检查范围没有符合条件的文件，不代表整个应用没有占用。</p>
                      )}
                    <ReasonList title="扫描期间保留（文件或目录项）" reasons={row.reasons ?? []} />
                    {row.skipped > 0 && !row.reasons?.length && (
                      <small>
                        {row.skipped} {ui.skippedSafety}
                      </small>
                    )}
                  </>
                ) : (
                  <p>尚未检查；扫描不会删除文件。</p>
                )}
              </details>
              {!terminal &&
                !scan?.incomplete &&
                !scan?.unsettled &&
                ['running', 'process_unknown'].includes(item?.status ?? '') && (
                  <button
                    className="result-link"
                    disabled={model.busy || model.saving}
                    onClick={model.refreshAvailability}
                  >
                    重新检查应用状态
                  </button>
                )}
            </div>
            <div className="rule-amount">
              <strong>
                {terminal
                  ? outcome
                    ? formatRecordedBytes(outcome.deleted_bytes)
                    : '—'
                  : row && row.phase !== 'pending'
                    ? !measured
                      ? accounting
                        ? scanAmount(accounting, 'observed')
                        : '未取得'
                      : editable
                        ? `已选 ${formatBytes(amount?.bytes ?? 0n)}`
                        : `已识别 ${accounting ? scanAmount(accounting, 'observed') : formatBytes(row.bytes)}`
                    : '—'}
              </strong>
              <small>
                {terminal
                  ? outcome
                    ? `已删除 ${formatResultCount(outcome.deleted)} 个文件`
                    : '未纳入计划'
                  : row
                    ? !measured
                      ? '文件数量未取得'
                      : `${row.files + (row.observation?.retained_files ?? 0)} 个文件${editable ? ` · 可选 ${formatBytes(row.bytes)}` : ''}`
                    : '等待扫描'}
              </small>
              {!catalog && !terminal && editable && (
                <small>{amount?.files ? `已选 ${amount.files} 个文件` : '未选择'}</small>
              )}
            </div>
            <button
              className="button secondary small details-button"
              disabled={!canDetail}
              aria-label={dynamic.viewFiles(rule.title)}
              onClick={() =>
                model.openDetail(
                  rule,
                  terminal && !outcome ? 'unplanned' : terminal ? 'cleanup' : 'scan',
                )
              }
            >
              查看文件
              <ChevronRight size={15} />
            </button>
          </article>
        );
      })}
    </div>
  );
}
