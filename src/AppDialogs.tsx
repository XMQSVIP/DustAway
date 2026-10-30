import * as AlertDialog from '@radix-ui/react-alert-dialog';
import { Trash2 } from 'lucide-react';
import { useEffect, useRef, type RefObject } from 'react';
import { api } from './api';
import { CoverageNotice } from './CoverageNotice';
import { useDialogFocus } from './dialogFocus';
import { ExclusionsDialog } from './ExclusionsDialog';
import { FilesDialog, type FileViewState } from './FilesDialog';
import { dynamic, formatBytes, text as t, ui } from './i18n';
import { ManualCart } from './ManualCart';
import { preparedSummary } from './SelectionSummary';
import type { AppController } from './useAppController';

export function AppDialogs({
  model,
  cleanupTrigger,
  onConfirmCleanup: confirmCleanup,
}: {
  model: AppController;
  cleanupTrigger: RefObject<HTMLElement | null>;
  onConfirmCleanup: () => void;
}) {
  const {
    boot,
    detail,
    detailQuery,
    summaryRows,
    excluded,
    selected,
    ready,
    busy,
    saving,
    drafts,
    addDraft,
    setDetail,
    setDraftDialog,
    selectFiles,
    reports,
    draftDialog,
    error,
    pending,
    uncertain,
    saveDrafts,
    setDrafts,
    rescanDialog,
    setRescanDialog,
    guard,
    startScan,
    plans,
    setPlans,
    clearDialog,
    setClearDialog,
    setHistory,
    setHistoryEpoch,
  } = model;
  const alertFocus = useDialogFocus();
  const cleanupFocus = useDialogFocus(cleanupTrigger);
  const prepared = preparedSummary(plans);
  const scanDisks = [
    ...new Set(
      plans
        .filter(({ plan }) => plan.categories.length > 0)
        .flatMap(({ scope }) => model.jobs[scope]?.scan_volumes ?? []),
    ),
  ];
  const fileViews = useRef(new Map<string, FileViewState>());
  const terminalKey = reports.map((r) => `${r.id}:${r.finished_at}:${r.status}`).join('|');
  useEffect(() => {
    fileViews.current.clear();
  }, [model.sessionKey, terminalKey]);
  return (
    <>
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
                const { files, directories, bytes } = prepared;
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
              <strong>{prepared.amount}</strong>
              <span>{prepared.counts}</span>
            </div>
            <p className="muted">
              以上是所选文件大小，不是承诺释放的磁盘空间。文件不会进入回收站。
            </p>
            {prepared.ruleFiles > 0 && (
              <p className="confirm-rule-summary">
                垃圾扫描已选：{prepared.ruleFiles.toLocaleString()} 个文件 ·{' '}
                {formatBytes(prepared.ruleBytes)}
                <br />
                扫描范围：
                {scanDisks.length ? scanDisks.join('、') : '未记录磁盘范围，请核对文件明细'}
                {scanDisks.length > 1 && '（不表示每个磁盘都有所选文件）'}
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
    </>
  );
}
