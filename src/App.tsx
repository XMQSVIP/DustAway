import { SoftwarePage } from './SoftwarePage';
import { uninstallActive, uninstallLabels } from './softwareTypes';
import { formatResultCount } from './cleanupResults';
import * as AlertDialog from '@radix-ui/react-alert-dialog';
import {
  AppWindow,
  ArrowRight,
  ChevronRight,
  CircleAlert,
  Download,
  ExternalLink,
  FolderOpen,
  HardDrive,
  History,
  LoaderCircle,
  Monitor,
  Moon,
  Plus,
  Search,
  Settings as SettingsIcon,
  ShieldCheck,
  SlidersHorizontal,
  Sun,
  Trash2,
  Wind,
  X,
} from 'lucide-react';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { AnalysisPanel, analysisStates } from './AnalysisPanel';
import { CleanupDecisionPanel, CleanupOutcomes } from './CleanupDecisionPanel';
import { CoverageNotice } from './CoverageNotice';
import { ExclusionsDialog } from './ExclusionsDialog';
import { FilesDialog, type FileViewState } from './FilesDialog';
import { selectionSummary } from './SelectionSummary';
import { ReportRow } from './ReportReceipt';
import { resultTitle } from './ResultCounts';
import { SettingsPage } from './SettingsPage';
import { HistoryPage } from './HistoryPage';
import { TitleBar } from './TitleBar';
import { api, desktop } from './api';
import { useDialogFocus } from './dialogFocus';
import { dynamic, formatBytes, formatTime, text as t, ui } from './i18n';
import { useSpatialPreferences } from './spatialPreferences';
import { useAppController } from './useAppController';
import { HomeStorage } from './HomeStorage';
import { CleanupSpaceResult } from './CleanupSpaceResult';
import { ManualCart } from './ManualCart';
import { PurposeGroups } from './PurposeGroups';
import { ResultReviewPanel } from './ResultReviewPanel';
import type { ResultStatus, ResultTarget } from './cleanupResults';
import { cleanupPresentation } from './cleanupPresentation';
import { AttachmentPanel } from './AttachmentPanel';
import { WeixinCleanup } from './WeixinCleanup';

export default function App() {
  const [attachmentsOpen, setAttachmentsOpen] = useState(false);
  const model = useAppController();
  const [wideLayout, setWideLayout] = useState(
    () => typeof matchMedia === 'function' && matchMedia('(min-width: 960px)').matches,
  );
  useEffect(() => {
    if (typeof matchMedia !== 'function') return;
    const media = matchMedia('(min-width: 960px)');
    const update = () => setWideLayout(media.matches);
    media.addEventListener('change', update);
    return () => media.removeEventListener('change', update);
  }, []);
  const mainRef = useRef<HTMLElement>(null);
  const pageScroll = useRef<Record<string, number>>({});
  useLayoutEffect(() => {
    if (mainRef.current) mainRef.current.scrollTop = pageScroll.current[model.tab] ?? 0;
  }, [model.tab]);
  const { preferences, updatePreferences } = useSpatialPreferences();
  const [settingsLeaf, setSettingsLeaf] = useState('appearance');
  const [inspectedRules, setInspectedRules] = useState<string[] | null>(null);
  const [fromAnalysis, setFromAnalysis] = useState(false);
  const alertFocus = useDialogFocus();
  const cleanupTrigger = useRef<HTMLElement | null>(null);
  const cleanupStarting = useRef(false);
  const cleanupFocus = useDialogFocus(cleanupTrigger);
  const reportRef = useRef<HTMLDetailsElement>(null);
  const resultsRef = useRef<HTMLDivElement>(null);
  const [helpOpen, setHelpOpen] = useState(false);
  const reviewRef = useRef<HTMLDivElement>(null);
  const [review, setReview] = useState<{ key: string; status: ResultStatus } | null>(null);
  const terminalKey = model.reports.map((r) => `${r.id}:${r.finished_at}:${r.status}`).join('|');
  const viewingKey = `${model.sessionKey}/${terminalKey}`;
  const reviewKey = `${terminalKey}/${model.sessionKey}`;
  const [dismissedReceipt, setDismissedReceipt] = useState('');
  const fileViews = useRef(new Map<string, FileViewState>());
  useEffect(() => {
    fileViews.current.clear();
  }, [model.sessionKey, terminalKey]);
  function finishViewing() {
    setDismissedReceipt(viewingKey);
    setReview(null);
    setFromAnalysis(false);
    model.setNotice('');
    model.setTab('clean');
    if (mainRef.current) mainRef.current.scrollTop = 0;
  }
  useEffect(() => setReview(null), [reviewKey]);
  useEffect(() => {
    if (reportRef.current) reportRef.current.open = false;
  }, [terminalKey, model.sessionKey]);
  function inspectRules(ids: string[]) {
    setInspectedRules([...ids]);
    requestAnimationFrame(() => {
      resultsRef.current?.focus({ preventScroll: true });
      resultsRef.current?.scrollIntoView({ block: 'start' });
    });
  }
  function showReport() {
    if (reportRef.current) {
      reportRef.current.open = true;
      reportRef.current.focus({ preventScroll: true });
      reportRef.current.scrollIntoView({ block: 'start' });
    }
  }
  function openResult(target: ResultTarget) {
    const current = model.decision.targets.find(
      (t) => t.key === target.key && t.session === target.session,
    );
    const rule = model.boot?.rules.find((r) => r.id === current?.ruleId);
    if (current?.session && rule)
      model.openDetail(rule, 'cleanup', current.status, current.reportId);
    else showReport();
  }
  function reviewResult(status: ResultStatus | null = model.decision.reviewStatus) {
    if (!status) {
      showReport();
      return;
    }
    const targets = model.decision.targets.filter((t) => t.status === status);
    if (targets.length === 1 && targets[0].session) {
      openResult(targets[0]);
      return;
    }
    if (!targets.length) {
      showReport();
      return;
    }
    setReview({ key: reviewKey, status });
    requestAnimationFrame(() => {
      reviewRef.current?.focus({ preventScroll: true });
      reviewRef.current?.scrollIntoView({ block: 'start' });
    });
  }
  function runPrimary() {
    switch (primaryKind) {
      case 'scan':
        void model.scan();
        break;
      case 'prepare':
        beginCleanup();
        break;
      case 'refresh_apps':
        void model.refreshAvailability();
        break;
      case 'verify':
        model.retryStatus();
        break;
      case 'stop':
        model.stopTask();
        break;
      case 'review_unknown':
        showReport();
        break;
      case 'finish':
        finishViewing();
        break;
      case 'analysis':
        model.setTab('analysis');
        break;
      case 'choose':
        resultsRef.current?.focus({ preventScroll: true });
        resultsRef.current?.scrollIntoView({ block: 'start' });
        break;
    }
  }
  function beginCleanup() {
    if (cleanupStarting.current || model.busy || model.saving) return;
    cleanupStarting.current = true;
    cleanupTrigger.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    void model.prepare().finally(() => {
      cleanupStarting.current = false;
      requestAnimationFrame(() => {
        const trigger = cleanupTrigger.current;
        if (
          !document.querySelector('[role="alertdialog"]') &&
          trigger?.isConnected &&
          !trigger.closest('[hidden]') &&
          !trigger.matches(':disabled')
        )
          trigger.focus({ preventScroll: true });
      });
    });
  }
  function confirmCleanup() {
    if (
      (!model.plans.length && !model.windowsMaintenance.plans.length) ||
      model.busy ||
      model.saving
    )
      return;
    cleanupTrigger.current = null;
    model.setTab('clean');
    void model.execute();
  }
  const {
    boot,
    setInitEpoch,
    initializing,
    initError,
    checking,
    setHistoryEpoch,
    historyError,
    historyLoading,
    tab,
    setTab,
    selected,
    history,
    setHistory,
    error,
    setError,
    analysis,
    sessions,
    reports,
    uncertain,
    stopping,
    retryStatus,
    reconcile,
    notice,
    setNotice,
    setHandoffSession,
    pending,
    closing,
    plans,
    setPlans,
    detail,
    setDetail,
    detailQuery,
    openDetail,
    excluded,
    setExpanded,
    clearDialog,
    setClearDialog,
    drafts,
    setDrafts,
    draftDialog,
    setDraftDialog,
    rescanDialog,
    setRescanDialog,
    sessionKey,
    busy,
    ready,
    summaryRows,
    scanned,
    candidateBytes,
    candidateCount,
    guard,
    stopTask,
    selectFiles,
    startScan,
    scan,
    prepare,
    execute,
    saving,
    save,
    addExclusion,
    addDraft,
    saveDrafts,
    currentJob,
  } = model;
  const receiptDismissed =
    !!(terminalKey || sessionKey) &&
    dismissedReceipt === viewingKey &&
    !model.hasManual &&
    !busy &&
    !uncertain &&
    checking === 'ready';
  const idleHome =
    receiptDismissed ||
    (!scanned &&
      !model.hasManual &&
      !busy &&
      !reports.length &&
      !fromAnalysis &&
      model.decision.action === 'scan');
  const presentation = cleanupPresentation(model);
  const cleanStage = idleHome ? 'idle' : presentation.stage;
  const emptyResult =
    cleanStage === 'review' &&
    model.windowsMaintenance.selected.size === 0 &&
    !busy &&
    !model.hasManual &&
    !model.decision.items.some(
      (item) =>
        item.capability === 'allowed' &&
        ['ready', 'deselected'].includes(item.status) &&
        item.foundFiles > 0,
    );
  const primaryKind = idleHome ? 'scan' : emptyResult ? 'finish' : model.decision.action;
  const analysisSelectionView =
    tab === 'analysis' &&
    !!analysis &&
    analysis.state !== 'scanning' &&
    !currentJob &&
    !uncertain &&
    checking === 'ready';
  const chosenSummary = selectionSummary(model);
  const sidebarReview = wideLayout && tab === 'clean' && cleanStage === 'review';
  const darkTheme =
    boot?.settings.theme === 'dark' ||
    ((!boot || boot.settings.theme === 'system') &&
      typeof matchMedia === 'function' &&
      matchMedia('(prefers-color-scheme: dark)').matches);
  const primaryAction = (
    <button
      className="button primary"
      onClick={runPrimary}
      disabled={
        !boot ||
        initializing ||
        !!initError ||
        !desktop ||
        saving ||
        pending ||
        closing ||
        stopping ||
        (busy && !['verify', 'stop'].includes(model.decision.action)) ||
        (primaryKind === 'scan' && (!model.scanRuleIds.length || !model.canScanVolumes))
      }
    >
      {idleHome
        ? '开始扫描'
        : emptyResult
          ? '返回首页'
          : primaryKind === 'review_unknown'
            ? '查看清理记录'
            : model.decision.actionLabel}
      <ArrowRight size={17} />
    </button>
  );
  return (
    <div
      className="app-shell desk-app"
      data-page={tab}
      data-clean-stage={cleanStage}
      data-home={idleHome && tab === 'clean'}
      data-runtime={boot?.version.endsWith('+e2e-fixture') ? 'fixture' : 'standard'}
    >
      <a href="#main" className="skip-link">
        {ui.skipContent}
      </a>
      <TitleBar
        closing={closing}
        onError={setError}
        actions={
          <>
            <span className="local-mark">
              <i />
              本地运行
            </span>
            <button
              className="icon-button theme-toggle"
              aria-label={darkTheme ? '切换为浅色' : '切换为深色'}
              title={darkTheme ? '切换为浅色' : '切换为深色'}
              disabled={!desktop || !boot || saving || closing}
              onClick={() => void save({ theme: darkTheme ? 'light' : 'dark' })}
            >
              {darkTheme ? <Sun size={18} /> : <Moon size={18} />}
            </button>
          </>
        }
      >
        <div className="chapter-navigation">
          <nav aria-label={ui.mainNavigation}>
            {(
              [
                { id: 'clean', label: t.clean, icon: HardDrive },
                { id: 'analysis', label: '空间分析', icon: FolderOpen },
                { id: 'software', label: '软件卸载', icon: AppWindow },
                { id: 'history', label: t.history, icon: History },
                { id: 'settings', label: t.settings, icon: SettingsIcon },
              ] as const
            ).map((item) => (
              <button
                key={item.id}
                className={`nav-item ${tab === item.id ? 'active' : ''}`}
                aria-current={tab === item.id ? 'page' : undefined}
                onClick={() => setTab(item.id)}
              >
                <item.icon size={19} aria-hidden="true" />
                {item.label}
              </button>
            ))}
          </nav>
        </div>
      </TitleBar>
      <div className="content-column">
        <main
          id="main"
          className="main-content"
          tabIndex={-1}
          ref={mainRef}
          onScroll={(event) => {
            pageScroll.current[tab] = event.currentTarget.scrollTop;
          }}
        >
          {model.uninstall && uninstallActive(model.uninstall.phase) && tab !== 'software' && (
            <div className="message info" role="status">
              <span>
                {model.uninstall.software.name}：{uninstallLabels[model.uninstall.phase]}
              </span>
              <button className="button secondary small" onClick={() => setTab('software')}>
                查看卸载任务
              </button>
            </div>
          )}
          {analysis?.state === 'scanning' && tab !== 'analysis' && (
            <div className="message info" role="status">
              <span>
                {analysisStates[analysis.state]} · 已检查 {analysis.checked.toLocaleString()} 项
              </span>
              <button className="button secondary small" onClick={() => setTab('analysis')}>
                查看空间分析
              </button>
              <button
                className="button secondary small"
                disabled={pending || stopping || uncertain || closing}
                onClick={stopTask}
              >
                {stopping ? '停止中…' : '停止分析'}
              </button>
            </div>
          )}

          {error && tab !== 'settings' && (
            <div className="message error" role="alert">
              <CircleAlert size={19} />
              <span>{error}</span>
              <button
                className="icon-button"
                aria-label={ui.dismissError}
                onClick={() => setError('')}
              >
                <X size={17} />
              </button>
            </div>
          )}
          {notice && (
            <div className="message info" role="status">
              <CircleAlert size={19} />
              <span>{notice}</span>
              <button
                className="icon-button"
                aria-label={ui.dismissNotice}
                onClick={() => setNotice('')}
              >
                <X size={17} />
              </button>
            </div>
          )}
          {initializing && (
            <div className="message info" role="status">
              正在读取设置并核对后台状态…
            </div>
          )}
          {initError && (
            <div className="message error" role="alert">
              <span>启动数据加载失败：{initError}</span>
              <button className="button secondary small" onClick={() => setInitEpoch((v) => v + 1)}>
                重新加载
              </button>
            </div>
          )}
          {checking !== 'ready' && !initializing && (
            <div className="message info" role="status">
              <span>
                {checking === 'checking'
                  ? '正在核对请求是否已受理，请勿重复操作…'
                  : '暂时无法确认任务状态，请重新检查。'}
              </span>
              <button className="button secondary small" onClick={retryStatus}>
                重新核对请求
              </button>
            </div>
          )}
          {uncertain && !initializing && !initError && (
            <div className="message info" role="status">
              <span>暂时无法确认任务状态</span>
              <button className="button secondary small" onClick={retryStatus}>
                重新检查状态
              </button>
              <button className="button secondary small" onClick={() => setInitEpoch((v) => v + 1)}>
                重新加载
              </button>
            </div>
          )}
          {closing && (
            <div role="status" className="message info">
              <LoaderCircle className="spin" size={18} />
              {t.closing}
            </div>
          )}
          <div hidden={tab !== 'software'}>
            <SoftwarePage model={model} active={tab === 'software'} />
          </div>
          <div className="page-stage analysis-page" hidden={tab !== 'analysis'}>
            <AnalysisPanel
              manualChoices={model.manualChoices}
              onSelectManual={model.selectManual}
              onSelectManualBatch={model.selectManualBatch}
              selectionTarget={model.analysisTarget}
              onExcludeManual={model.excludeManual}
              journey={model.journey}
              taskNotice={
                uncertain || checking !== 'ready'
                  ? '暂时无法确认任务受理状态，请先核对，不要重复启动。'
                  : error
              }
              onCheckTask={uncertain || checking !== 'ready' ? retryStatus : undefined}
              active={tab === 'analysis' && !!boot && !initError}
              snapshot={analysis}
              excludedRules={boot?.settings.excluded_rules ?? []}
              busy={busy}
              stopping={stopping || pending || uncertain || closing}
              onStop={stopTask}
              onStart={(token) =>
                guard(async () => {
                  await api.startAnalysis(token);
                  await reconcile();
                })
              }
              onCleanup={(ids) => {
                inspectRules(ids);
                setExpanded(new Set(ids.map((id) => id.split('.')[0])));
                setFromAnalysis(true);
                setTab('clean');
                setNotice('正在查看对应类别，已有勾选保持不变。查看不会改变选择。');
              }}
            />
          </div>
          <section className="page-stage cleanup-page" hidden={tab !== 'clean'}>
            <header className="page-heading cleanup-heading" hidden={idleHome}>
              <div>
                <h1>清理</h1>
              </div>
              <button
                className="button secondary small attachment-heading-action"
                disabled={busy || saving || !desktop}
                onClick={() => setAttachmentsOpen(true)}
              >
                聊天附件整理
              </button>
              <button className="analysis-shortcut surface" onClick={() => setTab('analysis')}>
                <Search size={20} />
                <span>
                  <strong>查找大文件</strong>
                  <small>按大小查找文件和文件夹，按需选择</small>
                </span>
                <ChevronRight size={20} />
              </button>
            </header>
            {fromAnalysis && inspectedRules && (
              <section className="analysis-handoff action-folio" aria-label="从分析继续处理">
                <p>
                  查看这些类别：
                  {inspectedRules
                    .map((id) => boot?.rules.find((r) => r.id === id)?.title ?? id)
                    .join('、')}
                  。重新全面扫描后可查看最新候选；清理范围按规则决定，可能涉及多个磁盘。
                </p>
                <div className="inline-actions">
                  <button
                    className="button secondary small"
                    disabled={busy}
                    onClick={() => {
                      const allowed = inspectedRules.filter(
                        (id) => !boot?.settings.excluded_rules.includes(id),
                      );
                      if (!allowed.length) {
                        setNotice('对应类别已在设置中排除，请先核对设置。');
                        return;
                      }
                      setHandoffSession(sessionKey);
                      setPlans([]);
                      setNotice('全面扫描会检查所有未排除的支持类别；请重新扫描后选择清理文件。');
                    }}
                  >
                    准备重新全面扫描
                  </button>
                  <button
                    className="result-link"
                    onClick={() => {
                      setTab('analysis');
                      requestAnimationFrame(() => {
                        const origin = model.journey.returnFocus.current;
                        const list = document.querySelector<HTMLElement>('.analysis-list');
                        const target =
                          origin?.isConnected && !origin.closest('[hidden]')
                            ? origin
                            : list && !list.closest('[hidden]')
                              ? list
                              : document.getElementById('main');
                        target?.focus({ preventScroll: true });
                      });
                    }}
                  >
                    返回刚才的分析位置
                  </button>
                </div>
              </section>
            )}
            <div className="attachment-entry" hidden={!idleHome}>
              <button
                className="button secondary small"
                disabled={busy || saving || !desktop}
                onClick={() => setAttachmentsOpen(true)}
              >
                聊天附件整理
              </button>
            </div>
            {attachmentsOpen && <AttachmentPanel onClose={() => setAttachmentsOpen(false)} />}
            <WeixinCleanup model={model} onAttachments={() => setAttachmentsOpen(true)} />
            <CleanupDecisionPanel
              model={model}
              weixinSeparate
              home={receiptDismissed}
              docked={!wideLayout && cleanStage === 'review'}
              resultsRef={resultsRef}
              onReview={reviewResult}
              primaryAction={primaryAction}
              reviewAction={
                cleanStage === 'review' ? (
                  <>
                    {!busy && model.decision.action !== 'scan' && sessions.length > 0 && (
                      <button
                        className="result-link"
                        disabled={
                          !model.scanRuleIds.length || !model.canScanVolumes || !desktop || saving
                        }
                        onClick={scan}
                      >
                        {t.rescan}
                      </button>
                    )}
                  </>
                ) : undefined
              }
            />
            {!idleHome && review?.key === reviewKey && (
              <ResultReviewPanel
                ref={reviewRef}
                status={review.status}
                targets={model.decision.targets.filter((target) => target.status === review.status)}
                onOpen={openResult}
                onReport={showReport}
              />
            )}
            {!idleHome && presentation.terminal && (
              <>
                <CleanupSpaceResult model={model} />
                <div className="inline-actions" aria-label="清理后的下一步">
                  <button className="result-link" onClick={showReport}>
                    查看结果
                  </button>
                  <button
                    className="result-link"
                    disabled={busy || saving || !model.canScanVolumes}
                    onClick={scan}
                  >
                    重新扫描
                  </button>
                  <button className="result-link" onClick={() => setTab('analysis')}>
                    查看大占用
                  </button>
                  <button className="result-link" onClick={() => void guard(() => api.storage())}>
                    Windows 存储设置
                  </button>
                </div>
              </>
            )}
            {!idleHome && reports.length > 0 && model.decision.result.state === 'unconfirmed' && (
              <button className="result-link" onClick={finishViewing}>
                返回首页
              </button>
            )}
            {!idleHome && !presentation.terminal && (
              <details className="space-fold">
                <summary>查看磁盘可用空间</summary>
                <HomeStorage
                  space={model.homeSpace}
                  pending={!model.journey.locationsLoaded || model.journey.locationsLoading}
                  unavailable={
                    !!model.journey.locationError ||
                    !model.journey.locations.some(
                      (l) => l.kind === 'volume' && l.supported !== false,
                    )
                  }
                />
              </details>
            )}
            {!idleHome && reports.length > 0 && (
              <details className="report-panel result-fold" ref={reportRef} tabIndex={-1}>
                <summary>查看清理记录</summary>
                <CleanupOutcomes model={model} onReview={reviewResult} />
                {reports.map((report) => (
                  <ReportRow
                    key={report.id}
                    report={report}
                    rules={boot?.rules ?? []}
                    canDetail={(id) =>
                      model.decision.targets.some(
                        (target) =>
                          target.reportId === report.id && target.ruleId === id && target.session,
                      )
                    }
                    onDetail={(id, status) => {
                      const rule = boot?.rules.find((r) => r.id === id);
                      if (rule) openDetail(rule, 'cleanup', status, report.id);
                    }}
                  />
                ))}
                {presentation.found.length > 0 && (
                  <section aria-label="各类别与未选择文件">
                    <h3>各类别与未选择文件</h3>
                    <PurposeGroups model={model} items={presentation.found} result />
                  </section>
                )}
                <details className="space-fold">
                  <summary>查看磁盘可用空间</summary>
                  <HomeStorage space={model.homeSpace} />
                </details>
              </details>
            )}
            <details className="more-cleanup-help" hidden={idleHome && !helpOpen}>
              <summary>其他处理方式与保留范围</summary>
              <div className="inline-actions">
                <button className="result-link" onClick={() => setTab('analysis')}>
                  查找空间大头
                </button>
                <button className="result-link" onClick={() => void guard(() => api.storage())}>
                  Windows 存储设置
                </button>
              </div>

              <p className="muted">
                近期文件按各类别的期限保留：临时文件为连续 24 小时，运行日志为 7 天，IIS
                历史日志与崩溃转储为 30 天。
              </p>
            </details>
            {drafts.length > 0 && (
              <div className="draft-banner">
                <span>长期排除草稿 · {drafts.length} 项，尚未保存</span>
                <button
                  className="button secondary small"
                  disabled={busy || saving}
                  onClick={() => setDraftDialog(true)}
                >
                  核对排除草稿
                </button>
              </div>
            )}
            <div className="safety-note">
              <ShieldCheck size={21} />
              <div>
                <strong>{t.safeTitle}</strong>
                <p>{t.safeBody}</p>
              </div>
            </div>
          </section>

          {tab === 'history' && <HistoryPage model={model} />}
          {tab === 'settings' && (
            <SettingsPage
              model={model}
              preferences={preferences}
              updatePreferences={updatePreferences}
              settingsLeaf={settingsLeaf}
              setSettingsLeaf={setSettingsLeaf}
            />
          )}
        </main>
        <footer
          className="action-bar task-dock"
          aria-label="任务与本次选择"
          hidden={
            sidebarReview ||
            (tab === 'clean' && presentation.terminal && !busy && !uncertain) ||
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
                    ? ui.working
                    : reports.length && !model.hasManual && !analysisSelectionView
                      ? `本次已删除 ${formatResultCount(model.decision.result.deleted)} 个文件 · ${formatResultCount(model.decision.result.directories.deleted)} 个文件夹 · ${model.decision.result.bytes === null ? '统计不可用' : formatBytes(model.decision.result.bytes)}`
                      : scanned || model.hasManual || analysisSelectionView
                        ? `已选 ${chosenSummary.amount} · ${chosenSummary.counts}`
                        : '扫描垃圾，腾出空间'}
            </span>
            <small
              data-important={analysisSelectionView && (candidateCount > 0 || model.preparing)}
            >
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
                        const selected =
                          document.querySelector<HTMLDetailsElement>('.manual-results');
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
                {cleanStage === 'review' && !wideLayout && primaryAction}
              </>
            )}
          </div>
        </footer>
      </div>
      <footer className="app-footnote">
        <span>
          <i />
          {desktop ? '本地处理 · 数据留在电脑' : '网页预览 · 扫描仅在桌面版运行'}
        </span>
        <span className="footnote-links">
          <button
            onClick={() => {
              setTab('clean');
              setHelpOpen(true);
              requestAnimationFrame(() => {
                const help = document.querySelector<HTMLDetailsElement>('.more-cleanup-help');
                if (help) {
                  help.open = true;
                  help.scrollIntoView({ block: 'start' });
                  help.querySelector('summary')?.focus();
                }
              });
            }}
          >
            帮助与保留范围
          </button>
          <span>拂尘 {boot?.version ?? ''}</span>
        </span>
      </footer>
      {detail && model.detailCurrent && (
        <FilesDialog
          key={`${detail.id}:${detailQuery.session}:${detailQuery.reportId}:${detailQuery.view}:${detailQuery.status}`}
          viewCache={fileViews.current}
          revision={summaryRows.find((r) => r.rule_id === detail.id)?.revision}
          initialView={detailQuery.view}
          initialStatus={detailQuery.status}
          rule={detail}
          scope={detail.requires_admin ? 'system' : 'user'}
          session={detailQuery.session}
          excluded={excluded}
          included={model.included}
          ruleSelected={selected.has(detail.id)}
          editable={
            ready &&
            model.decision.items.find((r) => r.rule.id === detail.id)?.capability === 'allowed' &&
            ['ready', 'deselected'].includes(
              model.decision.items.find((r) => r.rule.id === detail.id)?.status ?? '',
            ) &&
            !summaryRows.find((r) => r.rule_id === detail.id)?.blocked
          }
          actionsEnabled={!busy && !saving && !detail.requires_admin}
          drafts={drafts}
          onDraft={addDraft}
          onReviewDrafts={() => {
            setDetail(null);
            setDraftDialog(true);
          }}
          onSelect={selectFiles}
          cleanup={reports.length > 0}
          onClose={() => setDetail(null)}
        />
      )}
      {draftDialog && (
        <ExclusionsDialog
          error={error}
          drafts={drafts}
          saving={saving || pending || uncertain}
          onClose={() => setDraftDialog(false)}
          onSave={saveDrafts}
          onRemove={(index) => setDrafts((ds) => ds.filter((_, i) => i !== index))}
        />
      )}
      <AlertDialog.Root open={rescanDialog} onOpenChange={setRescanDialog}>
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="overlay" />
          <AlertDialog.Content className="dialog unfolded-sheet" {...alertFocus}>
            <AlertDialog.Title>还有未保存的排除草稿</AlertDialog.Title>
            <AlertDialog.Description>
              重新扫描会使当前候选及草稿失效，请先保存或明确放弃。
            </AlertDialog.Description>
            <div className="dialog-actions">
              <AlertDialog.Cancel className="button secondary">返回</AlertDialog.Cancel>
              <button
                className="button secondary"
                onClick={() => {
                  setDrafts([]);
                  setRescanDialog(false);
                  void guard(() => startScan(model.scanRuleIds));
                }}
              >
                放弃草稿并扫描
              </button>
              <button
                className="button primary"
                onClick={() => {
                  setRescanDialog(false);
                  setDraftDialog(true);
                }}
              >
                核对并保存
              </button>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
      <AlertDialog.Root
        open={plans.length > 0 || model.windowsMaintenance.plans.length > 0}
        onOpenChange={(open) => {
          if (!open) {
            setPlans([]);
            model.windowsMaintenance.setPlans([]);
          }
        }}
      >
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="overlay" />
          <AlertDialog.Content className="dialog confirm-dialog unfolded-sheet" {...cleanupFocus}>
            <div className="danger-icon">
              <Trash2 size={24} />
            </div>
            <AlertDialog.Title>{t.confirmTitle}</AlertDialog.Title>
            <AlertDialog.Description>{t.confirmDescription}</AlertDialog.Description>
            {model.windowsMaintenance.plans.length > 0 && (
              <section className="windows-confirmation">
                <h3>已选 Windows 维护项目</h3>
                <p>
                  先完成已选文件清理，再由 Windows 按以下类别整理。文件范围由 Windows
                  决定；官方估算单独计算。
                </p>
                {model.windowsMaintenance.plans
                  .flatMap((p) => p.plan.actions)
                  .map((action) => (
                    <p key={action.id}>
                      <strong>
                        {action.title} ·{' '}
                        {action.estimate_bytes === null
                          ? '估算未知'
                          : `约 ${formatBytes(action.estimate_bytes)}`}
                      </strong>
                      <br />
                      {action.impact}
                    </p>
                  ))}
              </section>
            )}
            {model.preparedSelection?.known &&
              (() => {
                const files = plans.reduce((n, item) => n + item.plan.files, 0);
                const directories = plans.reduce(
                  (n, item) => n + (item.plan.manual?.directories ?? 0),
                  0,
                );
                const bytes = plans.reduce((n, item) => n + BigInt(item.plan.bytes), 0n);
                const before = model.preparedSelection;
                const retained =
                  Math.max(0, before.files - files) + Math.max(0, before.directories - directories);
                return retained > 0 || bytes < before.bytes ? (
                  <p className="plan-adjustment" role="status">
                    核验后{retained > 0 ? `有 ${retained} 项已保留，` : '清理范围已更新，'}
                    仅删除下列清单中的内容。
                  </p>
                ) : null;
              })()}
            <div className="confirm-total">
              <strong>{formatBytes(plans.reduce((n, p) => n + BigInt(p.plan.bytes), 0n))}</strong>
              <span>
                {plans.reduce((n, p) => n + p.plan.files, 0).toLocaleString()} {ui.fileUnit} ·{' '}
                {plans.reduce((n, p) => n + (p.plan.manual?.directories ?? 0), 0)} 个文件夹
              </span>
            </div>
            {plans.some(({ plan }) => plan.files > (plan.manual?.files ?? 0)) && (
              <p className="confirm-rule-summary">
                垃圾扫描已选：
                {plans.reduce((n, { plan }) => n + plan.files - (plan.manual?.files ?? 0), 0)}{' '}
                个文件 ·{' '}
                {formatBytes(
                  plans.reduce(
                    (n, { plan }) => n + BigInt(plan.bytes) - BigInt(plan.manual?.bytes ?? '0'),
                    0n,
                  ),
                )}
              </p>
            )}
            {plans
              .filter((p) => p.plan.manual)
              .map(({ plan }) => (
                <div key={plan.id} className="manual-confirm">
                  <strong>你自选的内容</strong>
                  <p>
                    {plan.manual!.files} 个文件 · {plan.manual!.directories} 个文件夹 ·{' '}
                    {formatBytes(plan.manual!.bytes)}
                  </p>
                  <ManualCart model={model} readonly />
                  <p>
                    清理其中的已选内容，文件夹可能保留。
                    {plan.manual!.retained > 0 && `另有 ${plan.manual!.retained} 项已保留。`}
                  </p>
                </div>
              ))}
            {plans.some((p) => (p.plan.categories ?? []).length || p.plan.impacts.length) && (
              <details>
                <summary>查看类别与影响</summary>
                <ul className="impact-list">
                  {plans
                    .flatMap((p) => p.plan.categories ?? [])
                    .map((c) => (
                      <li key={c.rule_id}>
                        <strong>
                          {boot?.rules.find((r) => r.id === c.rule_id)?.title ?? c.rule_id}
                        </strong>{' '}
                        · {dynamic.files(c.files)} · {formatBytes(c.bytes)}
                        {boot?.rules.find((r) => r.id === c.rule_id)?.cleanup_policy !==
                          'recommended' && (
                          <span className="admin-label">你手动选择的需确认项目</span>
                        )}
                        <CoverageNotice coverage={c.coverage} />
                      </li>
                    ))}
                  {[...new Set(plans.flatMap((p) => p.plan.impacts))].map((impact) => (
                    <li key={impact}>{impact}</li>
                  ))}
                </ul>
              </details>
            )}
            <div className="dialog-actions">
              <AlertDialog.Cancel className="button secondary">取消</AlertDialog.Cancel>
              <button className="button danger" disabled={busy || saving} onClick={confirmCleanup}>
                {t.confirm}
              </button>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
      <AlertDialog.Root open={clearDialog} onOpenChange={setClearDialog}>
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="overlay" />
          <AlertDialog.Content className="dialog unfolded-sheet" {...alertFocus}>
            <AlertDialog.Title>{ui.clearHistoryTitle}</AlertDialog.Title>
            <AlertDialog.Description>{ui.clearHistoryDescription}</AlertDialog.Description>
            <div className="dialog-actions">
              <AlertDialog.Cancel className="button secondary">{ui.cancel}</AlertDialog.Cancel>
              <button
                className="button danger"
                onClick={() => {
                  setClearDialog(false);
                  void guard(async () => {
                    await api.clearHistory();
                    setHistory([]);
                    setHistoryEpoch((v) => v + 1);
                  });
                }}
              >
                {ui.clearHistory}
              </button>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
    </div>
  );
}
