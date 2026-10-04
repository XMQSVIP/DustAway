import { CoverageNotice } from './CoverageNotice';
import { attachmentSourceLabel } from './attachmentTypes';
import { formatTime, ui } from './i18n';
import { ReasonList } from './ReasonList';
import { fullyDeleted, ResultCounts, resultTitle } from './ResultCounts';
import type { Report } from './types';
import {
  cleanupResultSummary,
  formatRecordedBytes,
  recordedBytes,
  type ResultStatus,
} from './cleanupResults';

export function ReportRow({
  report,
  rules,
  onDetail,
  canDetail,
}: {
  report: Report;
  rules: import('./types').Rule[];
  onDetail?: (rule: string, status: ResultStatus) => void;
  canDetail?: (rule: string) => boolean;
}) {
  const result = cleanupResultSummary([report]);
  const attachmentLabel = attachmentSourceLabel(report.manual?.rule_id);
  return (
    <article className="report-row">
      <div className="report-top">
        <span className={`status-pill ${fullyDeleted(report) ? 'success' : ''}`}>
          {resultTitle(report)}
        </span>
        <span>
          {attachmentLabel
            ? `${attachmentLabel}附件整理`
            : report.scope === 'system'
              ? ui.systemCleanup
              : ui.userCleanup}
        </span>
        <time>{formatTime(report.started_at)}</time>
      </div>
      <div className="report-stats">
        <div>
          <small>{ui.deletedSize}</small>
          <strong>{formatRecordedBytes(report.deleted_bytes)}</strong>
        </div>
        <div>
          <small>{ui.candidateSize}</small>
          <strong>{formatRecordedBytes(report.candidate_bytes)}</strong>
        </div>
        <div className="report-volume-summary" aria-label="本次记录的磁盘空间变化">
          <small>本次记录的磁盘空间变化</small>
          {report.volume_space_changes?.length ? (
            <div className="volume-changes">
              {report.volume_space_changes.map((v, i) => (
                <div key={`${v.volume}-${i}`}>
                  <small>{v.volume} · 当前用户可用空间</small>
                  <strong>
                    {v.change_bytes === null
                      ? '无法计算'
                      : `${(recordedBytes(v.change_bytes, true) ?? 0n) > 0n ? '+' : ''}${formatRecordedBytes(v.change_bytes, true)}`}
                  </strong>
                  <small>
                    清理前{' '}
                    {v.before_bytes === null ? '未取得' : formatRecordedBytes(v.before_bytes)} →
                    清理后 {v.after_bytes === null ? '未取得' : formatRecordedBytes(v.after_bytes)}
                  </small>
                  {v.error && <small>采样不完整，不能将空间变化视为零。</small>}
                </div>
              ))}
            </div>
          ) : (
            <>
              <small>{ui.availableSpaceChange}</small>
              <strong>
                {report.free_space_change === null
                  ? '—'
                  : formatRecordedBytes(report.free_space_change, true)}
              </strong>
              <small>
                {report.free_space_change === null
                  ? '本次未取得磁盘空间采样'
                  : '旧报告使用系统盘口径'}
              </small>
            </>
          )}
        </div>
      </div>
      {result.state === 'unconfirmed' && (
        <p>部分结果尚未确认，原始记录已保留；不会自动重试删除。</p>
      )}
      <section aria-label="文件处理结果">
        <strong>文件</strong>
        <ResultCounts result={report} />
        {report.skipped > 0 && (
          <p className="muted">
            跳过的文件本次未删除，不计入实际删除量；可展开原因查看后续处理方式。
          </p>
        )}
      </section>
      <small className="muted">{ui.spaceCaveat}</small>
      {report.manual && (
        <section className="report-categories">
          <strong>{attachmentLabel ? `你选择的${attachmentLabel}附件` : '你自选的文件'}</strong>
          <ResultCounts result={report.manual} />
          <ReasonList title="未清理原因与后续处理" reasons={report.manual.reasons} showNextSteps />
        </section>
      )}
      {!!report.directories && (
        <section className="report-categories">
          <strong>文件夹</strong>
          <ResultCounts result={report.directories} unit="个文件夹" />
          <small>仅删除已核验且为空的文件夹；新增或保留内容不会被递归删除。</small>
        </section>
      )}
      {!onDetail && <p className="muted">历史仅保留统计，不保存文件路径明细。</p>}
      {report.categories?.length ? (
        <div className="report-categories">
          {report.categories.map((c) => (
            <div key={c.rule_id}>
              <strong>
                {rules.find((r) => r.id === c.rule_id)?.title ??
                  (c.rule_id === 'windows.system_temp' ? '系统临时文件' : '历史清理类别')}
              </strong>
              <ResultCounts
                result={c}
                onDetail={
                  onDetail && (canDetail?.(c.rule_id) ?? true)
                    ? (status) => onDetail(c.rule_id, status)
                    : undefined
                }
              />
              {onDetail && canDetail && !canDetail(c.rule_id) && (
                <p className="muted">当前会话明细不可用，仅保留统计；不会读取其他会话的文件。</p>
              )}
              <ReasonList title="执行结果原因与后续处理" reasons={c.reasons} showNextSteps />
              <CoverageNotice coverage={c.coverage} />
              <ReasonList title="扫描期间保留（不属于删除计划）" reasons={c.retained} />
            </div>
          ))}
        </div>
      ) : !report.manual ? (
        <p className="muted">旧版未记录原因明细及扫描完整程度</p>
      ) : null}
    </article>
  );
}
