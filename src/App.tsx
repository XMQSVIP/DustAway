import {
  AppWindow,
  ArrowRight,
  ChevronRight,
  FolderOpen,
  HardDrive,
  History,
  Moon,
  Search,
  Settings as SettingsIcon,
  ShieldCheck,
  Sun,
} from 'lucide-react';
import { lazy, Suspense, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { DeferredPage } from './DeferredPage';
import { AppDialogs } from './AppDialogs';
import { AppNotices } from './AppNotices';
import { CleanupDecisionPanel, CleanupOutcomes } from './CleanupDecisionPanel';
import { CleanupSpaceResult } from './CleanupSpaceResult';
import { CleanupTaskDock } from './CleanupTaskDock';
import { HistoryPage } from './HistoryPage';
import { HomeStorage } from './HomeStorage';
import { PurposeGroups } from './PurposeGroups';
import { ReportRow } from './ReportReceipt';
import { ResultReviewPanel } from './ResultReviewPanel';
import { SettingsPage } from './SettingsPage';
import { TitleBar } from './TitleBar';
import { WeixinCleanup } from './WeixinCleanup';
import { api, desktop } from './api';
import { cleanupPresentation } from './cleanupPresentation';
import type { ResultStatus, ResultTarget } from './cleanupResults';
import { text as t, ui } from './i18n';
import { useSpatialPreferences } from './spatialPreferences';
import { useAppController } from './useAppController';

const AnalysisPanel = lazy(() =>
  import('./AnalysisPanel').then((m) => ({ default: m.AnalysisPanel })),
);
const SoftwarePage = lazy(() =>
  import('./SoftwarePage').then((m) => ({ default: m.SoftwarePage })),
);
const AttachmentPanel = lazy(() =>
  import('./AttachmentPanel').then((m) => ({ default: m.AttachmentPanel })),
);

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
  const cleanupTrigger = useRef<HTMLElement | null>(null);
  const cleanupStarting = useRef(false);
  const reportRef = useRef<HTMLDetailsElement>(null);
  const resultsRef = useRef<HTMLDivElement>(null);
  const [helpOpen, setHelpOpen] = useState(false);
  const reviewRef = useRef<HTMLDivElement>(null);
  const [review, setReview] = useState<{ key: string; status: ResultStatus } | null>(null);
  const terminalKey = model.reports.map((r) => `${r.id}:${r.finished_at}:${r.status}`).join('|');
  const viewingKey = `${model.sessionKey}/${terminalKey}`;
  const reviewKey = `${terminalKey}/${model.sessionKey}`;
  const [dismissedReceipt, setDismissedReceipt] = useState('');
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
    initializing,
    initError,
    checking,
    tab,
    setTab,
    error,
    setError,
    analysis,
    sessions,
    reports,
    uncertain,
    stopping,
    retryStatus,
    reconcile,
    setNotice,
    setHandoffSession,
    pending,
    closing,
    setPlans,
    openDetail,
    setExpanded,
    drafts,
    setDraftDialog,
    sessionKey,
    busy,
    scanned,
    guard,
    stopTask,
    scan,
    saving,
    save,
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
  const taskInDock =
    !!currentJob && ['scanning', 'cleaning', 'stopping', 'uncertain'].includes(cleanStage);
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
          <AppNotices model={model} />
          <div hidden={tab !== 'software'}>
            <DeferredPage active={tab === 'software'}>
              <SoftwarePage model={model} active={tab === 'software'} />
            </DeferredPage>
          </div>
          <div className="page-stage analysis-page" hidden={tab !== 'analysis'}>
            <DeferredPage active={tab === 'analysis'}>
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
            </DeferredPage>
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
            {attachmentsOpen && (
              <Suspense fallback={<p role="status">正在打开附件整理…</p>}>
                <AttachmentPanel onClose={() => setAttachmentsOpen(false)} />
              </Suspense>
            )}
            <CleanupDecisionPanel
              model={model}
              weixinSeparate
              home={receiptDismissed}
              docked={taskInDock || (!wideLayout && cleanStage === 'review')}
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
            <WeixinCleanup model={model} onAttachments={() => setAttachmentsOpen(true)} />
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
        <CleanupTaskDock
          model={model}
          cleanStage={cleanStage}
          idleHome={idleHome}
          terminal={presentation.terminal}
          wideLayout={wideLayout}
          taskInDock={taskInDock}
          primaryAction={primaryAction}
          onBeginCleanup={beginCleanup}
        />
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
      <AppDialogs model={model} cleanupTrigger={cleanupTrigger} onConfirmCleanup={confirmCleanup} />
    </div>
  );
}
