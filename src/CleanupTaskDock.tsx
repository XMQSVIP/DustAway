import { ArrowRight, Search } from 'lucide-react';
import type { ReactNode } from 'react';
import { desktop } from './api';
import type { DeskStage } from './cleanupPresentation';
import { formatResultCount } from './cleanupResults';
import { formatBytes, text as t, ui } from './i18n';
import { selectionSummary } from './SelectionSummary';
import type { AppController } from './useAppController';

export function CleanupTaskDock({
  model,
  cleanStage,
  idleHome,
  terminal,
  wideLayout,
  taskInDock,
  primaryAction,
  onBeginCleanup: beginCleanup,
}: {
  model: AppController;
  cleanStage: DeskStage;
  idleHome: boolean;
  terminal: boolean;
  wideLayout: boolean;
  taskInDock: boolean;
  primaryAction: ReactNode;
  onBeginCleanup: () => void;
}) {
  const {
    tab,
    setTab,
    analysis,
    currentJob,
    uncertain,
    checking,
    busy,
    reports,
    scanned,
    stopping,
    candidateCount,
    saving,
    pending,
    closing,
    stopTask,
    sessions,
    scan,
  } = model;
  const analysisSelectionView =
    tab === 'analysis' &&
    !!analysis &&
    analysis.state !== 'scanning' &&
    !currentJob &&
    !uncertain &&
    checking === 'ready';
  const chosenSummary = selectionSummary(model);
  const sidebarReview = wideLayout && tab === 'clean' && cleanStage === 'review';
  return (
    <footer
      className="action-bar task-dock"
      aria-label="任务与本次选择"
      hidden={
        sidebarReview ||
        (tab === 'clean' && terminal && !busy && !uncertain) ||
        (tab === 'clean' && idleHome) ||
        (tab !== 'clean' &&
          !analysisSelectionView &&
          !model.hasManual &&
          !currentJob &&
          !(scanned && !reports.length) &&
          analysis?.state !== 'scanning' &&
          !uncertain)
      }
    >
      <div className="tray-marker" aria-hidden="true">
        <i />
        <i />
        <i />
      </div>
      <div>
        <span>
          {model.preparing
            ? model.stoppingPreparation
              ? '正在停止准备…'
              : '正在核验并准备清理清单…'
            : stopping
              ? '停止中'
              : busy
                ? currentJob?.state === 'scanning'
                  ? '正在扫描'
                  : currentJob?.state === 'cleaning'
                    ? '正在清理'
                    : ui.working
                : reports.length && !model.hasManual && !analysisSelectionView
                  ? `本次已删除 ${formatResultCount(model.decision.result.deleted)} 个文件 · ${formatResultCount(model.decision.result.directories.deleted)} 个文件夹 · ${model.decision.result.bytes === null ? '统计不可用' : formatBytes(model.decision.result.bytes)}`
                  : scanned || model.hasManual || analysisSelectionView
                    ? `已选 ${chosenSummary.amount} · ${chosenSummary.counts}`
                    : '扫描垃圾，腾出空间'}
        </span>
        <small data-important={analysisSelectionView && (candidateCount > 0 || model.preparing)}>
          {model.preparing
            ? model.preparationElapsed > 0
              ? `已用 ${model.preparationElapsed} 秒`
              : '选择已保留'
            : analysisSelectionView && candidateCount > 0
              ? '包含垃圾扫描中已选的项目'
              : reports.length && !model.hasManual && !analysisSelectionView
                ? `${model.decision.result.title}，详情和保留原因可按需查看。`
                : cleanStage === 'review'
                  ? '扫描记录中的已选大小，确认前会重新核验。'
                  : analysisSelectionView
                    ? model.hasManual || candidateCount
                      ? '确认前会核验清理范围'
                      : '勾选不再需要的内容后清理'
                    : currentJob?.state === 'cleaning'
                      ? ui.cancelEffect
                      : ui.confirmationExplanation}
        </small>
      </div>
      <div className="action-buttons">
        {analysisSelectionView ? (
          <>
            {model.preparing ? (
              <button
                className="button secondary"
                disabled={model.stoppingPreparation}
                onClick={() => void model.cancelPreparation()}
              >
                {model.stoppingPreparation ? '正在停止准备' : '停止准备'}
              </button>
            ) : (
              <button
                className="result-link"
                disabled={busy || saving || (!model.hasManual && !candidateCount)}
                onClick={() => {
                  setTab('clean');
                  requestAnimationFrame(() => {
                    const selected = document.querySelector<HTMLDetailsElement>('.manual-results');
                    if (selected) selected.open = true;
                  });
                }}
              >
                查看已选
              </button>
            )}
            <button
              className="button primary"
              hidden={model.preparing}
              onClick={beginCleanup}
              disabled={
                !desktop || busy || saving || (!model.hasManual && !model.decision.canPrepare)
              }
            >
              清理已选
              <ArrowRight size={17} />
            </button>
          </>
        ) : tab !== 'clean' ? (
          <>
            {(currentJob || analysis?.state === 'scanning') && (
              <button
                className="button secondary"
                disabled={pending || closing || stopping || uncertain}
                onClick={stopTask}
              >
                {stopping ? '停止中…' : '停止任务'}
              </button>
            )}
            <button
              className="button secondary"
              onClick={() => setTab(analysis?.state === 'scanning' ? 'analysis' : 'clean')}
            >
              {analysis?.state === 'scanning'
                ? '查看当前分析'
                : currentJob
                  ? '查看清理任务'
                  : '查看已选'}
            </button>
          </>
        ) : (
          <>
            {cleanStage !== 'review' &&
              !busy &&
              model.decision.action !== 'scan' &&
              sessions.length > 0 && (
                <button
                  className="button secondary"
                  disabled={
                    !model.scanRuleIds.length || !model.canScanVolumes || !desktop || saving
                  }
                  onClick={scan}
                >
                  <Search size={17} />
                  {t.rescan}
                </button>
              )}
            {(taskInDock || (cleanStage === 'review' && !wideLayout)) && primaryAction}
          </>
        )}
      </div>
    </footer>
  );
}
