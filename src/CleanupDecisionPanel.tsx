import { useRef, type ReactNode, type RefObject } from 'react';
import { Check } from 'lucide-react';
import { formatBytes } from './i18n';
import type { AppController } from './useAppController';
import { formatResultCount, type ResultStatus } from './cleanupResults';
import { cleanupFollowupCounts, cleanupPresentation } from './cleanupPresentation';
import { PurposeGroups } from './PurposeGroups';
import { cleanupPurpose } from './cleanupPurposes';
import { selectionSummary } from './SelectionSummary';
import { LayerScene } from './LayerScene';
import { HomeStorage } from './HomeStorage';
import { ScanVolumes } from './ScanVolumes';
import { ManualCart } from './ManualCart';
import { OfficialCacheTools } from './UvCache';
import { ScanTotals } from './ScanTotals';
import { WindowsMaintenancePanel } from './WindowsMaintenance';
import { scanAccountingFor, scanAmount, retainedReasonText } from './scanAssessment';

export function CleanupDecisionPanel({
  model,
  onReview,
  primaryAction,
  resultsRef,
  reviewAction,
  home = false,
  docked = false,
  weixinSeparate = false,
}: {
  model: AppController;
  onReview: (status: ResultStatus) => void;
  primaryAction?: ReactNode;
  resultsRef?: RefObject<HTMLDivElement | null>;
  reviewAction?: ReactNode;
  home?: boolean;
  docked?: boolean;
  weixinSeparate?: boolean;
}) {
  const { decision, currentJob, busy, saving } = model;
  const presentation = cleanupPresentation(model);
  const { found, incomplete, available, official } = presentation;
  // Weixin has a persistent, independently visible section, including empty/retained rows.
  const elsewhere = (item: (typeof presentation.found)[number]) =>
    !weixinSeparate || !item.rule.id.startsWith('weixin.');
  const buckets = presentation.buckets.map((bucket) => ({
    ...bucket,
    items: bucket.items.filter(elsewhere),
  }));
  const running = presentation.running.filter(elsewhere);
  const processUnknown = presentation.processUnknown.filter(elsewhere);
  const retainedAccounting = scanAccountingFor(buckets[2].items, model.summaryRows);
  const resultsNode = useRef<HTMLDivElement | null>(null);
  const currentRule = decision.items.find((item) => item.rule.id === currentJob?.current_rule);
  const stage = home ? 'idle' : presentation.stage;
  const terminal = !home && presentation.terminal;
  const taskActive =
    !!currentJob && ['scanning', 'cleaning', 'stopping', 'uncertain'].includes(stage);
  const summary = selectionSummary(model);
  async function recheck(button: HTMLButtonElement) {
    await model.refreshAvailability();
    if (!button.isConnected && document.activeElement === document.body)
      resultsNode.current?.focus();
  }
  const title =
    stage === 'uncertain'
      ? '先确认任务状态'
      : stage === 'stopping'
        ? '正在停止，请稍候'
        : stage === 'scanning'
          ? '正在寻找可清理的垃圾'
          : stage === 'cleaning'
            ? '正在清理，留出空间'
            : stage === 'complete'
              ? '清理完成'
              : stage === 'attention'
                ? decision.result.state === 'unconfirmed'
                  ? '部分结果尚未确认'
                  : '清理已结束'
                : stage === 'review'
                  ? model.hasManual
                    ? '清理你选择的内容'
                    : incomplete.length
                      ? '扫描已结束，部分结果需重新检查'
                      : model.selectionEstimate.files > 0 || model.selectionEstimate.directories > 0
                        ? '扫描完成，选择要清理的内容'
                        : available.length
                          ? '发现可清理内容，请选择'
                          : official.length
                            ? '发现可由官方工具整理的缓存'
                            : processUnknown.length
                              ? '部分清理项的运行状态尚未确认'
                              : running.length
                                ? '部分清理项需退出应用后检查'
                                : found.length
                                  ? '本次发现的内容暂不清理'
                                  : incomplete.length
                                    ? '部分位置尚未检查完成'
                                    : '这次没有发现可直接清理的垃圾'
                  : '清理垃圾，腾出空间';
  const idle = stage === 'idle' && !!primaryAction;
  const volumePicker = (
    <ScanVolumes
      locations={model.journey.locations}
      selection={model.scanVolumeSelection}
      disabled={busy || saving}
      error={model.journey.locationError}
      loading={model.journey.locationsLoading}
      loaded={model.journey.locationsLoaded}
      onChange={model.setScanVolumeSelection}
      onReload={model.journey.reloadLocations}
    />
  );
  const scanJob = Object.values(model.jobs).find((job) => job.scan_volumes?.length);
  return (
    <section
      className="cleanup-decision"
      aria-label="清理决策总览"
      data-stage={stage}
      data-idle={idle}
    >
      {idle ? (
        <div className="sculpture-home">
          <div className="home-index">
            <span>01</span>空间整理 · 从容开始
          </div>
          <div className="home-copy">
            <h2 aria-label="把空间，慢慢展开。">
              把空间，
              <br />
              <em>慢慢展开。</em>
            </h2>
            <p>
              拂去那些不再需要的，
              <br />
              让在意的事物，自在舒展。
            </p>
          </div>
          <LayerScene />
          <div className="home-control">
            <HomeStorage
              space={model.homeSpace}
              pending={!model.journey.locationsLoaded || model.journey.locationsLoading}
              unavailable={
                !!model.journey.locationError ||
                !model.journey.locations.some((l) => l.kind === 'volume' && l.supported !== false)
              }
            />
            <div className="home-scan-scope">{volumePicker}</div>
            <div className="home-scan">
              {primaryAction}
              <small>自动选择建议项，确认后才清理</small>
            </div>
          </div>
        </div>
      ) : (
        <div className={`desk-hero${taskActive ? ' task-hero' : ''}`}>
          <div className="desk-hero-copy">
            <div className="desk-annotation">
              <span />
              {stage === 'idle'
                ? '给重要的东西，留出空间'
                : stage === 'scanning'
                  ? '只检查，不删除'
                  : terminal
                    ? '本次清理回执'
                    : '本次整理'}
            </div>
            <h2>{title}</h2>
            {((stage === 'review' && !docked) || terminal) && (
              <div className="desk-amount">
                <strong>
                  {terminal
                    ? decision.result.bytes === null
                      ? '统计不可用'
                      : formatBytes(decision.result.bytes)
                    : summary.amount}
                </strong>
                <span>
                  {terminal
                    ? `实际删除 · ${formatResultCount(decision.result.deleted)} 个文件`
                    : `本次已选 · ${summary.counts}`}
                </span>
              </div>
            )}
            <p className="decision-guidance" role="status">
              {stage === 'idle'
                ? '自动查找可清理的垃圾，重要项目由你决定。'
                : stage === 'scanning'
                  ? '扫描只检查文件，不会删除，可以随时停止。'
                  : stage === 'review' && decision.canPrepare
                    ? '已选项目可以清理。查看文件，可保留不想删除的内容。'
                    : decision.message}
            </p>
            {stage === 'review' && !docked && (
              <small>扫描记录中的已选大小，确认前会重新核验。</small>
            )}
            {stage === 'review' && model.manualPreview.error && (
              <button className="result-link" onClick={model.manualPreview.retry}>
                重试已选统计
              </button>
            )}
            {primaryAction && !docked && (
              <div className="idle-scan-action">
                {primaryAction}
                {stage === 'review' && reviewAction}
                {['idle', 'scanning'].includes(stage) && <span>扫描不会删除文件</span>}
              </div>
            )}
            {currentJob && taskActive && (
              <div className="decision-progress" role="status">
                {currentJob.state === 'scanning' ? (
                  <>
                    <span>
                      正在检查：
                      {currentRule
                        ? [cleanupPurpose(currentRule.rule.id)?.title, currentRule.rule.title]
                            .filter(Boolean)
                            .join(' · ')
                        : '准备检查'}
                    </span>
                    <span>
                      已检查 {(currentJob.scan_progress?.checked ?? 0).toLocaleString()} 项 · 已用{' '}
                      {Math.floor((currentJob.scan_progress?.elapsed_ms ?? 0) / 1000)} 秒
                    </span>
                    {(currentJob.scan_progress?.elapsed_ms ?? 0) -
                      (currentJob.scan_progress?.last_progress_ms ?? 0) >=
                      10000 && <span>仍在等待当前检查返回，可请求停止</span>}
                  </>
                ) : (
                  <span>
                    已处理 {currentJob.completed.toLocaleString()} /{' '}
                    {currentJob.total.toLocaleString()} 项
                  </span>
                )}
              </div>
            )}
            {currentJob && ['scanning', 'cleaning'].includes(stage) && (
              <div
                className="live-progress"
                role="progressbar"
                aria-label={stage === 'scanning' ? '扫描进行中，总量待确定' : '文件处理进度'}
                aria-valuemin={0}
                aria-valuemax={stage === 'cleaning' ? currentJob.total || undefined : undefined}
                aria-valuenow={
                  stage === 'cleaning' && currentJob.total ? currentJob.completed : undefined
                }
                data-indeterminate={stage === 'scanning'}
              >
                <span
                  style={
                    stage === 'cleaning'
                      ? {
                          width: `${currentJob.total ? Math.min(100, (currentJob.completed / currentJob.total) * 100) : 0}%`,
                        }
                      : undefined
                  }
                />
              </div>
            )}
          </div>
          {stage !== 'review' && !taskActive && <LayerScene stage={stage} />}
        </div>
      )}
      {!idle && !terminal && model.scanned && (
        <div className="scan-scope-summary">
          <p>本次扫描：{scanJob?.scan_volumes?.join('、') || '全部磁盘'}</p>
          {!busy && (
            <details>
              <summary>调整下次扫描磁盘</summary>
              {volumePicker}
            </details>
          )}
          {docked && reviewAction}
        </div>
      )}
      {!home &&
        !terminal &&
        !['cleaning', 'stopping', 'uncertain'].includes(stage) &&
        model.scanned && <ScanTotals model={model} />}
      {(terminal || idle) && (model.uv?.task || model.maintenance?.task) && (
        <OfficialCacheTools model={model} />
      )}
      {terminal ? (
        <>
          <CleanupOutcomes model={model} />
        </>
      ) : (
        !home &&
        !['cleaning', 'stopping', 'uncertain'].includes(stage) &&
        (model.scanned || model.hasManual || stage === 'scanning') && (
          <div className="review-workspace">
            <div
              className="desk-results"
              ref={(node) => {
                resultsNode.current = node;
                if (resultsRef) resultsRef.current = node;
              }}
              tabIndex={-1}
            >
              {!!incomplete.length && !['cleaning', 'stopping', 'uncertain'].includes(stage) && (
                <section className="incomplete-notice" aria-label="扫描异常摘要">
                  <p role="status">
                    {incomplete.length} 类检查未完成或结果已失效，容量仅统计已读取范围。
                  </p>
                  <details>
                    <summary>查看检查问题</summary>
                    <ul>
                      {incomplete.map((item) => (
                        <li key={item.rule.id}>
                          {item.rule.title}：{item.scan?.label ?? '检查未完成'}
                          {item.capability === 'readonly' ? '；尚未开放清理' : ''}
                        </li>
                      ))}
                    </ul>
                  </details>
                  <button
                    className="result-link"
                    disabled={busy || saving}
                    onClick={async (event) => {
                      const button = event.currentTarget;
                      await model.scan();
                      if (!button.isConnected && document.activeElement === document.body)
                        resultsNode.current?.focus();
                    }}
                  >
                    重新扫描
                  </button>
                  <p className="muted">可按原规则处理其他已核验项目；重新扫描不会开放只读清理。</p>
                </section>
              )}
              <section aria-label="建议清理" className="recommended-results">
                <div className="results-heading">
                  <h3>建议清理</h3>
                  {!!buckets[0].items.length && (
                    <button
                      className="result-link"
                      disabled={busy || saving || !model.ready}
                      onClick={() =>
                        model.selectRecommended(
                          buckets[0].items.map((item) => item.rule.id),
                          true,
                        )
                      }
                    >
                      选择建议项
                    </button>
                  )}
                </div>
                {buckets[0].items.length ? (
                  <PurposeGroups model={model} items={buckets[0].items} policy="recommended" />
                ) : (
                  <p className="empty-result">
                    {stage === 'scanning'
                      ? '已完成的检查结果会出现在这里。'
                      : '本次没有可直接清理的建议项。'}
                  </p>
                )}
              </section>
              {!!buckets[1].items.length && (
                <details className="result-section">
                  <summary>
                    可自行选择 · 可选{' '}
                    {formatBytes(buckets[1].items.reduce((sum, item) => sum + item.foundBytes, 0n))}
                    {' · '}
                    {buckets[1].items.length} 项
                  </summary>
                  <p className="muted">这些内容可能仍有用途，按需勾选。</p>
                  <PurposeGroups model={model} items={buckets[1].items} policy="review" />
                </details>
              )}
              <OfficialCacheTools model={model} />
              {model.hasManual && (
                <details className="result-section manual-results">
                  <summary>自选文件与文件夹</summary>
                  <ManualCart model={model} />
                </details>
              )}
              {!!running.length && (
                <div className="incomplete-notice" role="region" aria-label="退出应用后可重新检查">
                  <p>
                    有 {running.length}{' '}
                    个清理项的相关应用仍在运行。保存工作并正常退出后，可重新检查。
                  </p>
                  <p className="muted">{running.map((item) => item.rule.title).join('、')}</p>
                  <button
                    className="result-link"
                    disabled={busy || saving}
                    onClick={(event) => void recheck(event.currentTarget)}
                  >
                    退出后重新检查
                  </button>
                </div>
              )}
              {!!processUnknown.length && (
                <div className="incomplete-notice" role="region" aria-label="运行状态尚未确认">
                  <p>有 {processUnknown.length} 个清理项暂时无法确认运行状态，已保留。</p>
                  <p className="muted">
                    {processUnknown.map((item) => item.rule.title).join('、')}
                  </p>
                  <button
                    className="result-link"
                    disabled={busy || saving}
                    onClick={(event) => void recheck(event.currentTarget)}
                  >
                    重新检查运行状态
                  </button>
                </div>
              )}
              {!!buckets[2].items.length && (
                <details className="result-section retained-results">
                  <summary>
                    <span>本次未清理 · {buckets[2].items.length} 类</span> · 已保留{' '}
                    {scanAmount(retainedAccounting, 'retained')}
                    {retainedAccounting.reasons.length > 0 &&
                      ` · ${retainedReasonText(retainedAccounting.reasons)}`}
                  </summary>
                  <p className="muted">
                    这里统计的是清理类别，不是文件数量。这些类别当前未纳入清理，不计入已选容量；展开可查看原因和后续处理方式。
                  </p>
                  <p className="muted">{cleanupFollowupCounts(buckets[2].items)}</p>
                  <PurposeGroups model={model} items={buckets[2].items} />
                </details>
              )}
            </div>
          </div>
        )
      )}
      {model.preparing && model.tab === 'clean' && (
        <div role="status" className="manual-cart">
          {model.stoppingPreparation ? '正在停止准备…' : '正在核验并准备清理清单…'}
          {model.preparationElapsed > 0 && <span>已用 {model.preparationElapsed} 秒</span>}
          <button
            className="button secondary"
            disabled={model.stoppingPreparation}
            onClick={() => void model.cancelPreparation()}
          >
            {model.stoppingPreparation ? '正在停止准备' : '停止准备'}
          </button>
        </div>
      )}
      {!home && (
        <div className="system-scan-entry">
          {model.boot?.capabilities.system_cleanup && (
            <button
              disabled={busy || saving || !model.ready}
              onClick={() => void model.scanSystem()}
            >
              扫描系统垃圾
            </button>
          )}
          <small>
            {model.boot?.capabilities.system_cleanup
              ? '单独授权检查 Windows 临时目录和标准 IIS 日志；首次不预选。'
              : '免安装版提供用户目录清理；完整系统扫描需使用安装版。'}
          </small>
        </div>
      )}
      {!home && <WindowsMaintenancePanel model={model} />}
    </section>
  );
}

/** Detailed outcome actions live inside the optional cleanup record. */
export function CleanupOutcomes({
  model,
  onReview,
}: {
  model: AppController;
  onReview?: (status: ResultStatus) => void;
}) {
  const { decision } = model;
  const interactive = !!onReview;
  return (
    <div className="decision-outcomes" aria-label={interactive ? '按结果查看文件' : '本次结果摘要'}>
      {model.reports.some((r) => r.directories) && (
        <span>
          已删除 {formatResultCount(decision.result.directories.deleted)} 个空文件夹
          {' · '}保留 {formatResultCount(decision.result.directories.skipped)} 个文件夹
        </span>
      )}
      {(
        [
          ['unknown', '状态不确定', decision.result.unknown],
          ['failed', '失败', decision.result.failed],
          [
            'skipped',
            decision.result.expectedRetention ? '按规则保留' : '跳过并保留',
            decision.result.skipped,
          ],
          ['candidate', '未处理', decision.result.unprocessed],
        ] as const
      ).map(
        ([status, label, count]) =>
          (count > 0 || !Number.isFinite(count)) &&
          (interactive ? (
            <button key={status} className="result-link" onClick={() => onReview?.(status)}>
              {label} {formatResultCount(count)} 项
            </button>
          ) : (
            <span key={status}>
              {label} {formatResultCount(count)} 项
            </span>
          )),
      )}
      {decision.result.stopped && <span>任务已停止或中断</span>}
      {decision.result.state === 'complete' && (
        <span>
          <Check size={16} />
          本次清理已完成。
        </span>
      )}
    </div>
  );
}
